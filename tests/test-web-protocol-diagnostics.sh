#!/usr/bin/env bash
# Test that the panel helper relays a bounded, sanitized tail of the
# installer's stderr after a failed protocol migration, and nothing on success.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf -- "$WORK"' EXIT
export TMPDIR="$WORK"
# shellcheck disable=SC1090
source <(sed '/^main "\$@"$/d' "$ROOT/amneziawg-web/scripts/amneziawg-web-privileged")
require_root() { :; }
resolve_awg_install_script() { printf '%s\n' "$WORK/mock-installer.sh"; }

fail_test() {
    echo "FAIL: $1"
    [[ ! -e "$WORK/stderr" ]] || cat "$WORK/stderr"
    exit 1
}

run_helper() {
    local rc=0
    (trap cleanup EXIT; main "$1") > "$WORK/stdout" 2> "$WORK/stderr" || rc=$?
    printf '%s' "$rc"
}

KEY='A/b+AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA='
LONG="$(head -c 300 /dev/zero | tr '\0' x)"
cat > "$WORK/mock-installer.sh" <<MOCK
#!/usr/bin/env bash
printf 'stdout must never be relayed\n'
for i in \$(seq 1 20); do printf 'line %02d\n' "\$i" >&2; done
printf '\033[0;31mERROR: awg setconf rejected the server configuration\033[0m\n' >&2
printf 'PrivateKey = ${KEY}\n' >&2
printf 'Line unrecognized: \`HeaderProtectionKey = ${KEY}\`\n' >&2
printf 'Key is not the correct length or format: \`short\`\n' >&2
printf 'public key ${KEY} was rejected\r\n' >&2
printf '${LONG}\n' >&2
printf 'ERROR: a staged protocol configuration was rejected by the installed tooling or running module\n' >&2
exit 1
MOCK
chmod +x "$WORK/mock-installer.sh"

rc="$(run_helper enable-awg31)"
[[ "$rc" == 2 ]] || fail_test "expected exit 2 from a failed migration, got $rc"
[[ ! -s "$WORK/stdout" ]] || fail_test 'helper wrote to stdout on failure'
[[ "$(head -n 1 "$WORK/stderr")" == 'amneziawg-web-privileged: protocol migration failed' ]] \
    || fail_test 'first stderr line is not the fixed failure line'
relayed="$(grep -c '^installer: ' "$WORK/stderr" || true)"
[[ "$relayed" == 12 ]] || fail_test "expected 12 relayed lines, got $relayed"
[[ "$(wc -l < "$WORK/stderr")" == 13 ]] || fail_test 'stderr carries lines beyond the failure line and the relayed tail'
! grep -q "$KEY" "$WORK/stderr" || fail_test 'key material was relayed'
! grep -qi 'privatekey' "$WORK/stderr" || fail_test 'a PrivateKey line was relayed'
! grep -qi 'headerprotectionkey' "$WORK/stderr" || fail_test 'a HeaderProtectionKey line was relayed'
! grep -q 'correct length' "$WORK/stderr" || fail_test 'a malformed-key line was relayed'
! grep -q $'\e' "$WORK/stderr" || fail_test 'terminal escapes were relayed'
! grep -q $'\r' "$WORK/stderr" || fail_test 'carriage returns were relayed'
grep -qx 'installer: ERROR: awg setconf rejected the server configuration' "$WORK/stderr" \
    || fail_test 'colored installer line was not relayed in plain text'
grep -qx 'installer: public key \[redacted\] was rejected' "$WORK/stderr" \
    || fail_test 'key-shaped value was not masked'
grep -qx "installer: $(head -c 240 /dev/zero | tr '\0' x) \[truncated\]" "$WORK/stderr" \
    || fail_test 'long line was not truncated to 240 characters'
[[ "$(tail -n 1 "$WORK/stderr")" == 'installer: ERROR: a staged protocol configuration was rejected by the installed tooling or running module' ]] \
    || fail_test 'last relayed line is not the installer verdict'
grep -qx 'installer: line 13' "$WORK/stderr" || fail_test 'tail dropped a line that fits the limit'
! grep -qx 'installer: line 12' "$WORK/stderr" || fail_test 'tail kept a line beyond the limit'
[[ -z "$(find "$WORK" -maxdepth 1 -name 'amneziawg-web-migration.*' -print -quit)" ]] \
    || fail_test 'captured stderr file was left behind'

printf '#!/usr/bin/env bash\nexit 3\n' > "$WORK/mock-installer.sh"
rc="$(run_helper disable-awg3)"
[[ "$rc" == 2 ]] || fail_test "expected exit 2 from a silent failure, got $rc"
[[ "$(wc -l < "$WORK/stderr")" == 1 ]] || fail_test 'a silent installer failure relayed lines'
[[ -z "$(find "$WORK" -maxdepth 1 -name 'amneziawg-web-migration.*' -print -quit)" ]] \
    || fail_test 'captured stderr file was left behind after a silent failure'

printf '#!/usr/bin/env bash\nprintf "WARNING: harmless\n" >&2\nexit 0\n' > "$WORK/mock-installer.sh"
rc="$(run_helper enable-awg31)"
[[ "$rc" == 0 ]] || fail_test "expected exit 0 from a successful migration, got $rc"
[[ ! -s "$WORK/stderr" && ! -s "$WORK/stdout" ]] || fail_test 'a successful migration produced output'
[[ -z "$(find "$WORK" -maxdepth 1 -name 'amneziawg-web-migration.*' -print -quit)" ]] \
    || fail_test 'captured stderr file was left behind after success'

echo 'PASS: panel helper relays sanitized installer diagnostics on failure only'
