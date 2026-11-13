#!/usr/bin/env bash
# Seeds the demo persona into the running guest, one stage at a time.
#
# usage: seed-demo.sh push MEDIA_DIR   copy demo/ and MEDIA_DIR/{photos,portraits} in, build,
#                                      and sign the seeder with RECON_SIGNING_IDENTITY
#        seed-demo.sh boot seeding     revert Recon's boot-args and reboot (AMFI enforced)
#        seed-demo.sh boot research    apply them again and reboot (readiness: eligible)
#        seed-demo.sh persona          account name and picture, computer name
#        seed-demo.sh ssh STEP...      steps that need Full Disk Access: check messages
#                                      documents
#        seed-demo.sh gui STEP...      steps that need privacy permission: contacts mycard
#                                      calendar reminders notes photos
#
# `gui` launches DemoSeeder.app in the guest's login session, so the first run of each
# step shows a macOS permission prompt in the VM window that someone (or vm-click.sh)
# must answer; the stage waits for the seeder and prints its log. Seeding expects the
# research-ready snapshot: an empty Photo Library and an empty chat.db.
#
# The gui steps must run between `boot seeding` and `boot research`. While
# amfi_get_out_of_my_way is active, every process carries CS_PLATFORM_BINARY, and tccd
# denies an app that is not Apple-signed without prompting ("CS_PLATFORM_BINARY set but
# not AppleSigned; prompt policy is Deny"), whatever its signature. The seeder is signed
# on the host with Recon's Developer ID, so the key never enters the guest and a grant
# survives rebuilding the seeder, which an ad-hoc signature's would not.
set -euo pipefail
here=$(dirname "$0")
source "$here/env.sh"

: "${GUEST_HOST:=$("$here/guest-ip.sh")}"
export GUEST_HOST
ssh_command="ssh -F /dev/null -i $VM_SSH_KEY -o IdentitiesOnly=yes -o IdentityAgent=none"
ssh_command+=" -o UserKnownHostsFile=$VM_WORKDIR/ssh/known_hosts"
seeder='~/demo/build/DemoSeeder.app'
arguments='--data ~/demo/data --media ~/DemoMedia/photos --portraits ~/DemoMedia/portraits'

stage=${1:?usage: seed-demo.sh push MEDIA_DIR | boot seeding|research | persona | ssh STEP... | gui STEP...}
shift
case "$stage" in
    push)
        media=${1:?usage: seed-demo.sh push MEDIA_DIR}
        identity=${RECON_SIGNING_IDENTITY:?set RECON_SIGNING_IDENTITY to a Developer ID Application identity}
        rsync -a --delete -e "$ssh_command" "$here/../demo/" "recon@$GUEST_HOST:demo/"
        rsync -a --delete -e "$ssh_command" "$media/" "recon@$GUEST_HOST:DemoMedia/"
        "$here/guest-ssh.sh" 'rm -f ~/Library/Logs/DemoSeeder.log; ~/demo/build-seeder.sh ~/demo/build'
        work=$(mktemp -d)
        rsync -a -e "$ssh_command" "recon@$GUEST_HOST:demo/build/DemoSeeder.app" "$work/"
        codesign --force --sign "$identity" "$work/DemoSeeder.app"
        rsync -a --delete -e "$ssh_command" "$work/DemoSeeder.app/" "recon@$GUEST_HOST:demo/build/DemoSeeder.app/"
        rm -rf "$work"
        "$here/guest-ssh.sh" "codesign -dv $seeder 2>&1 | grep -E '^(Identifier|TeamIdentifier)'"
        ;;
    boot)
        case ${1:?usage: seed-demo.sh boot seeding|research} in
            seeding) prepare='target/signed/recon ui session prepare-host --revert' ;;
            research) prepare='echo prepare-host | target/signed/recon ui session prepare-host' ;;
            *) echo "seed-demo: boot seeding|research" >&2; exit 2 ;;
        esac
        "$here/guest-ssh.sh" "cd ~/recon && $prepare >/dev/null && sudo shutdown -r now" || true
        sleep 10
        for _ in $(seq 60); do
            "$here/guest-ssh.sh" -o BatchMode=yes -o ConnectTimeout=3 true 2>/dev/null && break
            sleep 3
        done
        # SSH comes up before the automatic login has finished starting the GUI session.
        sleep 8
        "$here/guest-ssh.sh" 'echo "boot-args: [$(sysctl -n kern.bootargs)]"
            ~/recon/target/signed/recon ui session prepare-host --status | jq -r "\"readiness: \" + .readiness"'
        ;;
    persona)
        # The account keeps its short name `recon`, which Recon's paths depend on; only
        # what people see changes.
        "$here/guest-ssh.sh" "
            name=\$(jq -r '.contacts[] | select(.id == \"me\") | \"\\(.given) \\(.family)\"' ~/demo/data/contacts.json)
            portrait=\$(jq -r .me ~/demo/data/contact-photos.json)
            sips -s format jpeg -Z 512 ~/DemoMedia/portraits/\"\$portrait\" --out /tmp/account-picture.jpeg >/dev/null
            sudo cp /tmp/account-picture.jpeg /Library/User\\ Pictures/demo-persona.jpeg
            sudo dscl . -create /Users/recon RealName \"\$name\"
            sudo dscl . -delete /Users/recon JPEGPhoto 2>/dev/null || true
            sudo dscl . -create /Users/recon Picture /Library/User\\ Pictures/demo-persona.jpeg
            sudo scutil --set ComputerName \"\${name%% *}'s Mac\"
            echo \"account: \$(dscl . -read /Users/recon RealName | tail -1 | xargs), computer: \$(scutil --get ComputerName)\""
        ;;
    ssh)
        # Messages and its agents hold chat.db open and cache it; they relaunch on demand.
        "$here/guest-ssh.sh" "killall Messages imagent IMDPersistenceAgent 2>/dev/null; sleep 1
            $seeder/Contents/MacOS/DemoSeeder $* $arguments"
        ;;
    gui)
        # `open -W` loses track of the seeder, so the stage follows its log instead: it
        # ends with "done:" or "FAILED" after the lines that were there at launch.
        log='~/Library/Logs/DemoSeeder.log'
        start=$("$here/guest-ssh.sh" "touch $log; wc -l < $log" | xargs)
        "$here/guest-gui.sh" "open $seeder --args $* $arguments"
        while :; do
            sleep 5
            last=$("$here/guest-ssh.sh" "tail -n +$((start + 1)) $log")
            if grep -qE ' (done|FAILED)' <<<"$last"; then break; fi
        done
        tail -n 15 <<<"$last"
        grep -q ' done: ' <<<"$last"
        ;;
    *)
        echo "seed-demo: unknown stage '$stage'" >&2
        exit 2
        ;;
esac
