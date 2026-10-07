#!/usr/bin/env bash
#
# Let a user read RAPL energy counters without sudo, permanently.
#
#   sudo packaging/linux/install-permissions.sh [USER]   # install (USER defaults to the sudo caller)
#   sudo packaging/linux/install-permissions.sh --remove # uninstall
#
# Creates the "powercap" group, installs a udev rule granting it read access
# to the counters (see 60-ecocode-powercap.rules), applies it immediately and
# adds USER to the group. Security note: members of the group can read
# fine-grained energy counters, which can leak information about other
# processes on the machine (CVE-2020-8694). Only add trusted users.
set -euo pipefail

GROUP=powercap
RULE_NAME=60-ecocode-powercap.rules
RULE_DST=/etc/udev/rules.d/$RULE_NAME
RULE_SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$RULE_NAME"

if [[ $EUID -ne 0 ]]; then
    echo "This script changes system permissions; run it with sudo:" >&2
    echo "  sudo $0 ${*:-}" >&2
    exit 1
fi

reload_udev() {
    if command -v udevadm >/dev/null; then
        udevadm control --reload-rules
        udevadm trigger --subsystem-match=powercap --action=add
    else
        echo "udevadm not found: the rule will apply after the next reboot." >&2
    fi
}

if [[ "${1:-}" == "--remove" ]]; then
    rm -f "$RULE_DST"
    reload_udev
    echo "Removed $RULE_DST. Counters return to root-only after the next reboot."
    echo "The '$GROUP' group was kept; remove it with: groupdel $GROUP"
    exit 0
fi

user="${1:-${SUDO_USER:-}}"

getent group "$GROUP" >/dev/null || groupadd --system "$GROUP"
install -D -m 0644 "$RULE_SRC" "$RULE_DST"
reload_udev
echo "Installed $RULE_DST."

if [[ -n "$user" && "$user" != "root" ]]; then
    usermod -aG "$GROUP" "$user"
    echo "Added '$user' to the '$GROUP' group."
    echo "Log out and back in (or run 'newgrp $GROUP') for it to take effect, then check with: ecocode doctor"
else
    echo "No user given: add users with: usermod -aG $GROUP <user>"
fi
