#!/usr/bin/env bash
# Counts what the demo seeding left in each app's own store, over SSH (the guest's SSH
# sessions have Full Disk Access). Each query reads a copy of the store with its WAL,
# which holds the recent writes and which an immutable open would skip, so the apps'
# own files are never opened. A store whose schema differs from what is queried here
# reports "unavailable" rather than failing the rest.
#
# usage: verify-demo.sh
set -euo pipefail
here=$(dirname "$0")

"$here/guest-ssh.sh" '
query() {
    local database=$1 sql=$2
    local copy
    copy=$(mktemp -d)
    cp "$database" "$copy/store" 2>/dev/null && {
        cp "$database-wal" "$copy/store-wal" 2>/dev/null
        cp "$database-shm" "$copy/store-shm" 2>/dev/null
        sqlite3 "$copy/store" "$sql" 2>/dev/null
    }
    rm -rf "$copy"
}
count() {
    printf "%-16s %s\n" "$1" "$(query "$2" "$3" || true)"
}
group=~/Library/Group\ Containers
count contacts ~/Library/Application\ Support/AddressBook/AddressBook-v22.abcddb \
    "select count(*) from ZABCDRECORD where ZFIRSTNAME is not null or ZORGANIZATION is not null"
count calendars "$group/group.com.apple.calendar/Calendar.sqlitedb" \
    "select group_concat(title, \", \") from Calendar where title in (\"Home\", \"Work\", \"Family\", \"Fitness\", \"Travel\")"
count events "$group/group.com.apple.calendar/Calendar.sqlitedb" \
    "select count(*) from CalendarItem i join Calendar c on i.calendar_id = c.ROWID where c.title in (\"Home\", \"Work\", \"Family\", \"Fitness\", \"Travel\")"
# Reminders keeps one store per account, plus a local one; the seeded lists can be in any.
reminders=0
for store in "$group"/group.com.apple.reminders/Container_v1/Stores/Data-*.sqlite; do
    found=$(query "$store" "select count(*) from ZREMCDREMINDER where ZMARKEDFORDELETION = 0" || true)
    reminders=$((reminders + ${found:-0}))
done
printf "%-16s %s\n" reminders "$reminders"
count notes "$group/group.com.apple.notes/NoteStore.sqlite" \
    "select count(*) from ZICCLOUDSYNCINGOBJECT where ZTITLE1 is not null and ZMARKEDFORDELETION = 0"
count photos ~/Pictures/Photos\ Library.photoslibrary/database/Photos.sqlite \
    "select count(*) from ZASSET where ZTRASHEDSTATE = 0"
count albums ~/Pictures/Photos\ Library.photoslibrary/database/Photos.sqlite \
    "select group_concat(ZTITLE, \", \") from ZGENERICALBUM where ZKIND = 2 and ZTRASHEDSTATE = 0"
count messages ~/Library/Messages/chat.db "select count(*) from message"
count conversations ~/Library/Messages/chat.db "select count(*) from chat"
count attachments ~/Library/Messages/chat.db "select count(*) from attachment"
printf "%-16s %s\n" documents "$(find ~/Documents ~/Desktop ~/Downloads -type f ! -name ".*" | wc -l | xargs)"
printf "%-16s %s\n" account "$(dscl . -read /Users/recon RealName | tail -1 | xargs) on $(scutil --get ComputerName)"
'
