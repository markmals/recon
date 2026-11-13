# Demo environment

The `demo` snapshot is the `research-ready` guest with a fictional person's data in Apple's apps, like a demo Mac in an Apple Store. Recon inspections of Messages, Photos, Calendar, Contacts, Notes, or Reminders then meet realistic content (long lists, avatars, attachments, unread badges, repeating events) without anyone's real data in the guest or the repository.

Apple distributes its own retail demo content only to enrolled demo units, so this dataset was written for the purpose, following the scenes Apple's macOS marketing uses: a photographer's library, a family group chat, a trip being planned, a birthday coming up.

## The persona

Mia Navarro is a photographer and product designer in Los Angeles. The guest account's full name is "Mia Navarro" and the computer is "Mia's Mac". The short name stays `recon`, which Recon's paths depend on.

| App | Contents | Source |
| --- | --- | --- |
| Contacts | 20 cards, 17 with portraits, in the groups Tahoe Crew, Navarro Family, and Book Club; Mia's card is My Card | `data/contacts.json`, `data/contact-photos.json` |
| Calendar | 45 events on Home, Work, Family, Fitness, and Travel, from 14 days before seeding to 30 days after, with weekly, monthly, and yearly repeats | `data/calendar.json` |
| Reminders | 50 reminders in Groceries, Tahoe Trip, Studio, Home, and Gifts, with due dates, priorities, and completed items | `data/reminders.json` |
| Notes | 16 notes in 4 folders, On My Mac | `data/notes.json` |
| Photos | 669 items (600 photos, 69 videos) in 10 place albums, with favorites; the newest is from the day before seeding | the media directory, outside the repository |
| Messages | 16 conversations (13 one-to-one, 3 groups), 226 messages, 11 photo attachments, unread messages from the morning of seeding | `data/messages.json` |
| Finder | 10 PDF, DOCX, RTF, TXT, and CSV files in Documents, Desktop, and Downloads | `data/documents.json` |

Every date is relative to the day of seeding. The snapshot was seeded on 13 November 2026, and the guest's clock follows the host's, so the data's "today" drifts into the past; reseed to bring it back. Phone numbers are 555-01xx, email domains are `example.com`, `.net`, and `.org`, and private addresses, books, and businesses are invented. Only public venues are real.

## The media is not in the repository

The photos and portraits are Apple marketing stock photography from the experiment host, about 4.5 GB, and are deliberately not committed. `seed-demo.sh push MEDIA_DIR` copies `MEDIA_DIR/photos` and `MEDIA_DIR/portraits` into the guest. Photo file names carry the place tags (`demo-la`, `sicily`, `tahoe`, …) that `PhotosSeeder.swift` turns into albums and Messages attachments are chosen by; portrait file names are the ones `contact-photos.json` lists. On the experiment host the directory is `~/ReconResearchVM/demo-media`.

## Seeding

`DemoSeeder/` is a small AppKit app that `build-seeder.sh` compiles in the guest. Each step writes one app's data:

| Step | How | Needs |
| --- | --- | --- |
| `check` | Decodes every data file and cross-checks ids, calendars, photo tags, and participants | nothing |
| `contacts`, `calendar`, `reminders`, `photos` | Contacts, EventKit, PhotoKit | the app's privacy prompt |
| `notes`, `mycard` | AppleScript to Notes and Contacts | an Automation prompt each |
| `messages` | SQLite writes to `~/Library/Messages/chat.db`, which has no public API | Full Disk Access, which the guest's SSH sessions have |
| `documents` | Files written with AppKit and Core Graphics | nothing |

From a clean `research-ready` guest:

```sh
cd experiments/vm-research-environment/scripts
./vm-snapshot.sh reset research-ready && ./vm-lifecycle.sh boot
RECON_SIGNING_IDENTITY='Developer ID Application: …' ./seed-demo.sh push ~/ReconResearchVM/demo-media
./seed-demo.sh ssh check
./seed-demo.sh boot seeding     # AMFI enforced again, so privacy prompts can appear
./seed-demo.sh persona
./seed-demo.sh gui contacts calendar reminders notes mycard photos   # answer 6 prompts in the VM window
./seed-demo.sh ssh messages documents
./seed-demo.sh boot research    # boot-args back, readiness: eligible
./verify-demo.sh
```

Then, in the VM window, open Photos, Notes, and Reminders once and dismiss their welcome, iCloud, and notification sheets, and click away the notification banners. Leave notifications off: an alert that appears during an observation counts as UI interference. Over SSH, delete `~/DemoMedia`, `~/demo`, and `~/Library/Logs/DemoSeeder.log`, and empty `~/Library/Application Support/recon/inspection` except `prepare-host.lock`. Finally shut down, quit VirtualBuddy, and run `./vm-snapshot.sh save demo`.

The run that produced the snapshot took about eight minutes from reset to verified data: 63 s to push, about three minutes of prompts, a one-minute Photos import, 14 s for Messages and the documents, and about 20 s for each reboot.

### Why seeding drops the research boot-args

While `amfi_get_out_of_my_way` is active, every process carries `CS_PLATFORM_BINARY`, and `tccd` refuses to prompt for any app that Apple did not sign ("CS_PLATFORM_BINARY set but not AppleSigned; prompt policy is Deny"). The seeder's Photos request came back denied without a dialog, both ad hoc and signed with Recon's Developer ID ([`demo-seeding.txt`](../evidence/demo-seeding.txt), filed as `recon-pr1`). `boot seeding` runs `prepare-host --revert` and reboots, so the prompts appear normally. `boot research` reapplies `prepare-host` afterwards, and the grants and the seeded data survive both reboots. The seeder is still signed on the host with Recon's Developer ID, so the key never enters the guest and a grant survives rebuilding the seeder.

### What the app stores need

- **Messages** shows a row only when `attributedBody` holds `NSArchiver` data of an attributed string carrying `__kIMMessagePartAttributeName`; `text` alone displays nothing. Inserting a chat fires a trigger that calls the SQL function `verify_chat`, which exists only inside Messages, so the seeder registers a no-op one. Attachment rows need `original_guid`.
- **Contacts** stores an image saved by a third-party app but derives no thumbnail from it, and Contacts and Messages draw avatars from the thumbnail. The seeder sets one through `CNMutableContact`'s private `thumbnailImageData` setter.
- **My Card**: Setup Assistant creates a card from the account's original name. `mycard` deletes the card it replaces.
- **Verifying**: `verify-demo.sh` queries copies of each store with its write-ahead log. An immutable open skips the log and undercounts everything written recently.

## Known limitations

- Messages puts an "iMessage · Encrypted" header before every incoming message that follows an outgoing one. Setting `destination_caller_id`, setting `account` to the email, the phone number, or nothing, setting outgoing `handle_id` to 0, and setting incoming delivery fields did not change it. [INFERENCE] It follows from the guest having no signed-in iMessage account, which would need an Apple Account.
- The first time Messages opens a group chat, it adds an empty `any;+;chat…` twin, so `chat.db` holds 19 chats for 16 conversations. Each group still shows once.
- Every note carries the time of seeding, because Notes' AppleScript dictionary makes creation and modification dates read-only.
- Photos' background analysis (Memories, People) had run for about ten minutes when the snapshot was saved and continues from there on each boot.
- Recon cannot open a research session in Messages (`recon-efk`). Its sandbox denies the helper's `listen` and the unlink of its endpoint socket, and the open times out. Photos, Notes, Contacts, and Calendar pass the full research inspection on the seeded data ([`demo-seeding.txt`](../evidence/demo-seeding.txt)).

## Using it

```sh
cd experiments/vm-research-environment/scripts
./vm-snapshot.sh reset demo && ./vm-lifecycle.sh boot
./authorize-taskport.sh         # once per boot, before the first research attach
./inspect-target.sh photos /System/Applications/Photos.app launch research
```
