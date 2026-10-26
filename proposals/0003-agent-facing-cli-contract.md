---
id: proposal.0003
title: Agent-Facing CLI Contract
authors: [Mark Malstrom]
status: awaiting-implementation
pull-request: https://github.com/markmals/recon/pull/34
issues: []
supersedes: []
---

# Agent-Facing CLI Contract

## Summary

Give Recon's CLI a contract an unfamiliar agent can learn at runtime: help and version output, usage errors that name the offending argument, one machine-readable error object with a stable code per failure, a declarative command registry behind `recon schema`, and `recon context` reporting version, identity, permissions, and host capabilities.

## Authorization

- Authority: Mark consented on 2026-10-17, in conversation, to this proposal after reviewing a strategist analysis of Recon's CLI against external agent-first CLI guidance. No build mandate applies.
- Fit: This proposal is limited to the interface contract shared by every existing command — orientation, argument validation, error reporting, command discovery, and execution-context reporting. It changes no observation behavior and adds no capability that touches a target application.
- Decisions recorded: Mark selected one compact JSON error object on a single line as the default standard-error format on 2026-10-26. There is no prose prefix or alternate error-format mode.
- Approval: Mark explicitly approved implementation of this proposal on 2026-10-26 once that clarification was recorded, in Sesh session `5d3d3975-ea96-4a45-bc27-063741e02e03`. Approval includes the privacy-identity exception and the registry/error policy specified below. Acceptance of the finished implementation and permission to merge remain separate.
- Output-ownership revision: on 2026-10-26, Mark selected “Guarantee exactly one final response” in the parent approval question in Sesh session `5d3d3975-ea96-4a45-bc27-063741e02e03`. This explicitly authorizes parent-owned arbitration of both child output streams, discarding child output on a genuine wait failure, and making a post-spawn wait failure fatal for `context` rather than producing a fallback report. It grants no acceptance or merge authority.

## Motivation

An agent that has never seen Recon cannot use it without failing first.

Every orientation attempt is a failure. `recon`, `recon --help`, `recon -h`, `recon help`, `recon --version`, and `recon app` all print the same usage block to standard error and exit 1 (`crates/recon/src/main.rs`). There is no per-command help, and no way to learn Recon's version at all. An agent that wants to know what it is holding must read the repository.

Every mistake produces the same answer. `recon ui tree Calculator --window abc`, `recon ui screenshot Calculator` with no `--out`, `recon ui windows Calculator --bogus`, and `recon app list --json` each print the whole usage block and nothing about which argument was wrong (`crates/recon/src/ui/request.rs`). The agent is told that something in four lines of grammar was violated and left to guess which.

Every failure looks alike to a program. `main()` prints `recon: <message>` and exits 1, whatever happened. Ambiguity candidates are indented prose lines, a missing permission is a multi-line block naming a System Settings pane, and an accessibility timeout is a sentence. To choose between "pick one of these two process IDs", "ask a human for a permission", and "try again", an agent must match on English text and hope the wording never changes. Nothing distinguishes a transient failure from a permanent one.

Nothing is discoverable in a machine-readable form. The grammar lives in one hand-maintained `USAGE` string that already covers commands defined in another module. Recon has six commands today; the reverse-engineering families on the roadmap imply roughly forty more, each of which would extend that string by hand.

Execution context is discoverable only by failing, and failing has a side effect. An agent cannot learn whether Recon holds Screen Recording or Accessibility, whether this build is ad-hoc signed, whether the private interfaces Recon depends on still exist, or which version it is running, without running a `ui` command against a real application. Per decision 0001, a `ui` command that finds a permission missing also asks macOS to request it, which may put a dialog on a screen the agent cannot see.

The present workaround is prior knowledge: the agent must have read `guides/` or this repository. That is exactly the dependency an agent-facing workbench should not have, and it gets worse with every command added.

## Domain grounding

Claims are labelled **established** (authoritative external source), **repository evidence** (read from this repository), **inference**, or **hypothesis**. The strategist analysis that prompted this proposal is inference; every claim below was re-verified against its own source.

### Established context

**What agent-first guidance asks for**

- The guidance under evaluation states the principle "Make capabilities discoverable, inputs predictable, outputs structured, and effects verifiable", and defines an agent loop of Discover → Inspect → Validate → Preview → Execute → Verify. It recommends one authoritative command schema from which the parser, help, machine-readable metadata, and later MCP tool definitions are generated, so that "a flag advertised to an agent must actually work". For local developer tools it names a declarative command registry as the appropriate form. It asks that every failure carry a stable machine-readable error code, a concise explanation, details identifying the invalid input, a retryable/permanent distinction, and an appropriate process exit code, in a consistent JSON error schema with fields such as `code`, `message`, `retryable`, and `details`. It asks for an execution-context command such as `mycli context show`. Source: `/Users/orion/Documents/Documentation/Programming/agentic-dev/agent-first-clis.md`, based on [Cloudflare's `cf` CLI](https://blog.cloudflare.com/cloudflare-cf-cli-launch/).

**What established CLI convention actually says, and does not say**

- [clig.dev](https://clig.dev/) requires help on `-h` and `--help`, including for subcommands, and lists `myapp` with no arguments alongside `myapp --help` as a help trigger. It says "Send output to `stdout`… Anything that is machine readable should also go to `stdout`" and "Send messaging to `stderr`. Log messages, errors, and so on". Established.
- clig.dev does **not** state which stream help goes to, nor that `--help` exits zero, nor any numeric exit code. Its whole statement on exit codes is "Return zero exit code on success, non-zero on failure… Map the non-zero exit codes to the most important failure modes." Its Errors section says nothing about machine-readable or structured errors. Established by absence; this contradicts the common belief that clig.dev prescribes structured errors or a sysexits-style code table.
- clig.dev treats machine-readable output as opt-in: "Display output as formatted JSON if `--json` is passed." Established. Recon already departs from this: JSON on standard output is its only success format (proposal 0001).
- The [GNU Coding Standards](https://www.gnu.org/prep/standards/html_node/_002d_002dversion.html) state that `--version` "should direct the program to print information about its name, version, origin and legal status, all on standard output, and then exit successfully", that "the version number proper starts after the last space", and the same for [`--help`](https://www.gnu.org/prep/standards/html_node/_002d_002dhelp.html). Established.
- [POSIX.1-2017 §2.8.2](https://pubs.opengroup.org/onlinepubs/9699919799/utilities/V3_chap02.html) reserves exit status 127 for "command not found" and 126 for "found, but not an executable utility", and states that "the exit status of a command that terminated because it received a signal shall be reported as greater than 128". Established.
- BSD `sysexits.h` defines `EX_USAGE` 64 through `EX_CONFIG` 78. Its two live primary sources contradict each other: the current [FreeBSD man page](https://man.freebsd.org/cgi/man.cgi?query=sysexits&sektion=3) calls it "legacy exit status codes… deprecated and… retained only for compatibility. Its use is discouraged", while [Apple's man page](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/sysexits.3.html) calls the same table "preferable exit codes for programs", and the header shipped in this Mac's SDK carries no deprecation notice. Established, including the contradiction.
- clap, the dominant Rust argument parser, deliberately rejected `EX_USAGE` in favour of 2: its source comments "While sysexists.h defines EX_USAGE as 64, this doesn't seem to be used much in practice but instead 2 seems to be frequently used… Python's `argparse` returns 2", and defines `USAGE_CODE: i32 = 2` ([clap_builder source](https://raw.githubusercontent.com/clap-rs/clap/master/clap_builder/src/util/mod.rs), [issue 1327](https://github.com/clap-rs/clap/issues/1327)). [`Error::exit_code`](https://docs.rs/clap/latest/clap/error/struct.Error.html#method.exit_code): "When the error's kind would print to stderr this returns 2, else it returns 0" — help and version print to standard output and exit 0. Established.
- Version-string convention across comparable Rust and Go tools is a bare `MAJOR.MINOR.PATCH` followed by a parenthetical commit and date, not SemVer `+build` metadata. `cargo` formats `cargo {version}` then, when commit information exists, `" ({short_commit_hash} {commit_date})"`, giving `cargo 1.85.0 (d73d2caf9 2024-12-31)`; `rustc` prints `{binary} {version}` with the same shape and moves `commit-hash`, `commit-date`, and `host` behind `--verbose`; `gh` prints `gh version {version} ({buildDate})`. [SemVer 2.0.0](https://semver.org/) rule 10 defines `+` build metadata and requires it be ignored in precedence, but none of the three uses it. Established from each tool's source.

**What the parser and schema tooling can actually do**

- clap 4 (latest 4.6.7) errors on a repeated option by default: `ArgAction::Set` "will result in a `ArgumentConflict` unless `Command::args_override_self(true)` is set" ([docs](https://docs.rs/clap/latest/clap/builder/enum.ArgAction.html#variant.Set)). `multiple_occurrences` no longer exists in clap 4. Established; this is the opposite of the commonly assumed last-wins default.
- clap 4 interleaves options and positionals by default and has no setting to forbid it. Established ([derive tutorial](https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html)).
- An unknown flag and an extra positional both produce `ErrorKind::UnknownArgument`, distinguished by the offending string in `ContextKind::InvalidArg`. A hyphen-prefixed token is never consumed as an option value unless `Arg::allow_hyphen_values(true)` is set, which defaults to false. Established ([`ErrorKind`](https://docs.rs/clap/latest/clap/error/enum.ErrorKind.html), [`allow_hyphen_values`](https://docs.rs/clap/latest/clap/struct.Arg.html#method.allow_hyphen_values)).
- clap's built-in integer parser does **not** enforce digits only. `value_parser!(u32)` resolves to `RangedI64ValueParser`, which calls `str::parse::<i64>`, and Rust's integer `FromStr` accepts a leading plus: the standard library documents `assert_eq!(u32::from_str("+10"), Ok(10))` ([std docs](https://doc.rust-lang.org/std/primitive.u32.html#method.from_str)). A range such as `value_parser!(u32).range(1..)` is built in and enforces "greater than zero", but range-checking happens after parsing and does not reject `+12`. A custom value parser is required, and it is the digit test Recon already has. Established.
- clap errors are introspectable without scraping text: `Error::kind()` plus `Error::get(ContextKind)` yields the offending argument (`InvalidArg`), the invalid value and the valid values (`InvalidValue`, `ValidValue`), expected and actual value counts, and, for a missing required argument, the list of missing arguments. Established ([`Error`](https://docs.rs/clap/latest/clap/error/struct.Error.html), [`ContextKind`](https://docs.rs/clap/latest/clap/error/enum.ContextKind.html)).
- A built clap `Command` can be walked at runtime through public getters — `get_subcommands`, `get_arguments`, `get_value_names`, `get_default_values`, `get_possible_values`, `is_required_set`, `get_help` — and `Command::render_help` returns help as a value rather than printing it. `clap_complete` builds shell completions through exactly these getters, so reflection is a supported path rather than an accident. Established ([`Command`](https://docs.rs/clap/latest/clap/struct.Command.html), [`clap_complete` source](https://docs.rs/clap_complete/latest/src/clap_complete/aot/generator/utils.rs.html)).
- `schemars` 1.2.3 emits JSON Schema 2020-12 by default and honours `#[serde(rename_all)]` and `#[serde(flatten)]`. By default its schemas describe how a type is _deserialized_; `SchemaSettings::for_serialize()` selects the serialize contract instead, under which a field marked `#[serde(skip_serializing_if = …)]` is left out of `required` because serialization may omit it. Under the default deserialize contract that attribute does not affect presence. Established ([generating schemas](https://graham.cool/schemars/generating/), [schemars docs](https://docs.rs/schemars/latest/schemars/), [attribute reference](https://graham.cool/schemars/deriving/attributes/)).
- No maintained crate generates a JSON Schema from a `clap::Command`. Established.
- JSON Schema 2020-12 is the current dialect; its canonical `$schema` value is `https://json-schema.org/draft/2020-12/schema`, self-declared by the live meta-schema. json-schema.org states "The current version is 2020-12". A successor numbered release is in development and has not shipped. Established ([specification](https://json-schema.org/specification)).

**What macOS can report about Recon's own execution context**

- `CGPreflightScreenCaptureAccess` and `CGRequestScreenCaptureAccess` exist from macOS 10.15. Apple's reference pages are signature-only stubs; the "preflight checks without prompting, request may prompt" semantics are stated by Apple engineers in Apple-hosted [forum](https://developer.apple.com/forums/thread/732726) [threads](https://developer.apple.com/forums/thread/686498) rather than in reference prose. Established, with the weaker sourcing noted.
- `AXIsProcessTrusted` takes no options and has no prompting mechanism. Apple's own `AXUIElement.h` header documents `kAXTrustedCheckOptionPrompt` as the value "indicating whether the user will be informed if the current process is untrusted", adding "Prompting occurs asynchronously and does not affect the return value". Established from Apple's header text; the web reference page is a stub.
- Code-signing facts about the running process are available through public API: `SecCodeCopySelf`, then `SecCodeCopySigningInformation` with `kSecCSSigningInformation`. `kSecCodeInfoIdentifier` "is always present if the code is signed and always absent if the code is unsigned"; `kSecCodeInfoTeamIdentifier` carries the team identifier; `kSecCodeInfoFlags` carries the signature flags, in which `kSecCodeSignatureAdhoc = 0x0002` marks an ad-hoc signature. Established ([`SecCodeCopySigningInformation`](<https://developer.apple.com/documentation/security/seccodecopysigninginformation(_:_:_:)>), [`kSecCodeInfoIdentifier`](https://developer.apple.com/documentation/security/kseccodeinfoidentifier), [`CSCommon.h`](https://raw.githubusercontent.com/apple-oss-distributions/Security/main/OSX/libsecurity_codesigning/lib/CSCommon.h)). Bindings exist as `objc2-security` 0.3.2, the same family and major version as Recon's other Apple bindings.
- `ProcessInfo.operatingSystemVersion` is public, documented, and structured. `operatingSystemVersionString` is documented as "not appropriate for parsing". `kern.osproductversion` and `sw_vers` are not documented on developer.apple.com and are both subject to the SDK-compatibility version shim, under which a binary built against an older SDK is told an older product version. Established ([`operatingSystemVersion`](https://developer.apple.com/documentation/foundation/processinfo/operatingsystemversion), [`operatingSystemVersionString`](https://developer.apple.com/documentation/foundation/processinfo/operatingsystemversionstring)).
- There is no public API for the TCC responsibility relationship. Apple's Developer Technical Support states it directly: "The system has a 'responsibility' mechanism that it uses to track these relationships, and there's no public API for that facility" ([forum thread](https://developer.apple.com/forums/thread/95612)). `responsibility_spawnattrs_setdisclaim` is a private libSystem symbol whose declaration is mirrored in [LLVM's LLDB sources](https://raw.githubusercontent.com/llvm/llvm-project/main/lldb/source/Host/macosx/objcxx/PosixSpawnResponsible.h). Established.
- Apple documents no programmatic signal for "Spotlight indexing is unavailable". `mdutil -s <volume>` reports indexing status through a command-line tool whose output would have to be parsed, and `NSMetadataQuery`'s behaviour when indexing is off is not specified. Established by absence.

### Relevant constraints and principles

- Repository evidence: `VISION.md` says "Favor a Rust CLI and structured, durable experiment and result records that an agent can read, write, and revisit… Human readers need the same trail to be legible." Both halves of that sentence bind this proposal's error format.
- Repository evidence: `VISION.md` requires that "every claim remains traceable to observations, conditions, applications, and versions", which is why a version and an execution-context report are part of the contract rather than conveniences.
- Repository evidence: decision 0001 fixes Recon's privacy identity, the self-disclaiming re-execution, and the rule that "Commands that need no permission do not re-execute". It also states that Recon "checks a permission without prompting, asks macOS to request it when it is missing, and then fails". A command that reports permission state without requesting anything does not fit either sentence as written; see **Policies and decisions checked**.
- Repository evidence: proposals 0001 and 0002 fix the success contract — one compact JSON object on standard output, diagnostics on standard error, nonzero exit and no success JSON on failure, no silent truncation, no arbitrary target selection. This proposal must not disturb any of it.
- Repository evidence: `crates/recon/src/ui/request.rs` enforces six strictness rules — options before or after the target, repeated option rejected, unknown option rejected, missing or hyphen-prefixed option value rejected, numbers written as plain decimal digits only, `--pid` greater than zero — plus "an extra positional argument is an error" in `main.rs`. These are deliberate and must survive.
- Repository evidence: Recon's success types are `#[derive(Serialize)]` structs, except `Node`, which has a hand-written `Serialize` implementation, and `ElementNode`, which omits `valueTruncated` and `childrenOmitted` through `skip_serializing_if`. Any schema generator must be reconciled with both.
- Inference: `PROCESS.md` deliberately keeps the proposal, tests, guides, and code as independent cross-checks. A generated schema is compatible with that only if it is treated as code and checked against this proposal by tests and review, never as a second specification.
- Established: private interfaces can disappear in any macOS update, so the context report must describe what it found rather than what Recon expects to find.

### Quality bar

- An agent that has never seen Recon can, from `recon --help` alone, reach a correct invocation of any command without reading this repository, and can recover from any single failure using only the error object it received.
- Everything Recon advertises is true of the binary that advertised it: a flag in the schema works, a default in the schema is the runtime default, and an output schema validates the output the command actually produces.
- No failure requires reading English to classify. Code, retryability, and the data needed to recover are structured fields.
- Learning the execution context costs one command, changes nothing, and never puts a dialog on screen.
- The same text is legible to a person reading a terminal. Structured does not mean unreadable.

### Remaining uncertainty

- Apple's reference documentation does not state the non-prompting semantics of `CGPreflightScreenCaptureAccess`; the claim rests on Apple-hosted forum answers plus proposal 0002's direct observation that checking did not prompt. Hypothesis: that remains true on later macOS releases.
- Apple steers screen-capture clients toward ScreenCaptureKit, and there are secondary reports that macOS 15.1 began flagging `CGPreflightScreenCaptureAccess` usage to users. Whether a context command that preflights on every invocation is noticed by macOS is untested. Hypothesis.
- Whether a repeated, cheap `mdfind` probe is an acceptable way to report application-discovery availability on a machine where Spotlight is disabled, as against parsing `mdutil -s`, has not been measured.
- The exact per-`ErrorKind` context clap populates is established for the kinds Recon can provoke, but clap's `ErrorKind` is `#[non_exhaustive]`; a kind Recon does not anticipate must map to a defined code rather than escape the contract.

**Repository evidence from implementation, 2026-10-26:** clap's defaults accept a bare `-` and attached hyphen-prefixed string values such as `--out=--pid`, and can classify `--pid -1` as an unknown short option rather than a bad value of `--pid`. Typed value parsers must therefore own the hyphen rejection as well as the numeric constraints. This refines the parsing mechanism without changing the approved strictness rules.

## Existing baseline

Recon's own CLI is the baseline, and most of it is already what this proposal needs.

Preserve: one compact JSON object on standard output for success, and nothing else on that stream; diagnostics on standard error with a nonzero exit and no success JSON; ambiguity as an error listing candidates rather than an arbitrary pick; the effective target echoed in results; explicit truncation markers; the separation of read-only commands from `launch`; explicit failure on hosts other than macOS; no stdin prompts and bounded accessibility timeouts. All of it came from proposals 0001 and 0002 and survives unchanged.

Preserve, and promote from implementation detail to contract: the seven strictness rules in `request.rs` and `main.rs`. They are the reason Recon has never silently accepted `--pid +1` or quietly taken the last of two `--window` flags, and no replacement parser is acceptable if it loses one.

Replace: the single `USAGE` constant. It lives in `crates/recon/src/ui/request.rs`, describes the `app` commands defined in `main.rs`, is the response to every failure in `run()`'s match, and is printed to standard error with exit 1 for help requests. It is a hand-maintained string that is simultaneously the grammar, the help, and the error message, and it is already in the wrong module.

Replace: `eprintln!("recon: {error}"); exit(1)` in `main()`. Every failure in the crate is a `String`, so the type system permits no other outcome. Making errors structured is not a formatting change; it is a change to what a failure _is_ in this codebase.

Recon's closest prior art for machine-readable description is proposal 0002's output specifications, which are written as JSON shapes in prose and verified by tests. That works for five response types and does not survive forty commands.

## Proposed solution

Define one registry entry per command, and derive everything an agent needs from it.

Each entry declares the command's path, summary, arguments and options with their constraints, its effects, the permissions it may need, the hosts it supports, the type whose serialization is its output, and the error codes it can produce. The argument parser is built from that entry, so a command cannot exist outside the registry. Help is rendered from the same value. `recon schema` publishes it, with the output schema generated from the Rust type that actually serializes the output.

Failures stop being strings. Every failure carries a stable code, a human message, a retryable flag, and structured details, and is printed as one JSON object on standard error. Usage errors name the argument at fault and what would have been accepted, and exit 2; every other failure exits 1.

`recon context` answers "what am I holding, and what can it do here?" in one read-only command that checks permissions without requesting them.

```text
recon --help → orientation on stdout, exit 0, naming `recon schema` as the discovery path
recon schema → every command with its path, summary, and effects, plus the error contract
recon schema ui tree → arguments, constraints, effects, permissions, output schema, error codes
recon context → version, signing identity, permissions held, private interfaces available
recon ui tree Calculator --window abc → {"error":{"code":"INVALID_ARGUMENT",…}} on stderr, exit 2
recon ui screenshot Notes --out /tmp/n.png → PERMISSION_DENIED with the pane that grants it
```

## Detailed design

### Command grammar

- Recon has two kinds of command path. A **capability command** acts on the host and is written `recon <group> <operation> [<target>] [options]`; the existing six are `app list`, `app inspect`, `app launch`, `ui windows`, `ui tree`, and `ui screenshot`. A **meta-command** describes Recon itself and is written `recon <name> [arguments]`; this proposal defines `schema` and `context`.
- Group and operation names are kebab-case; option names are kebab-case and long-form only, written `--name <value>`. There are no short option names. Existing names do not change.
- A new capability command joins an existing group when it observes the same thing that group observes, and otherwise introduces a group. `ui` means the window server and Accessibility; `app` means bundles and processes.

### Orientation

- `recon --help`, `recon -h`, `recon help`, and `recon` with no arguments print top-level help to standard output and exit 0.
- `recon <group> --help`, `recon <group> -h`, and `recon help <group>` print that group's help; `recon <group> <operation> --help`, `-h`, and `recon help <group> <operation>` print that command's help. Both go to standard output and exit 0. The same applies to each meta-command.
- A help request is answered wherever it appears in the arguments and whatever else they contain: `recon ui tree --window abc --help` prints help and exits 0 rather than reporting the malformed value.
- Top-level help names the tool in one line, lists every command path with its one-line summary grouped under `app`, `ui`, and the meta-commands, lists `--help` and `--version`, and ends with a paragraph naming the discovery path: that every command prints one JSON object to standard output on success and one JSON error object to standard error on failure, that `recon schema` lists every command and the error contract, that `recon schema <command>` gives one command's full specification, and that `recon context` reports this installation's permissions and capabilities.
- Command help states the usage line, the summary, the description, each argument and option with its value name and constraints, the command's effects, the permissions it may need, the hosts it supports, and the error codes it can produce. It is rendered from the same registry entry `recon schema <command>` serializes, so the two cannot disagree.
- Help output contains no terminal escape sequences and is byte-identical whether or not standard output is a terminal.
- `recon --version` prints one line to standard output and exits 0, in the form `recon <version>` followed, when the build knows its commit, by ` (<short commit hash> <commit date>)` — for example `recon 0.1.0 (a3731c8 2026-10-17)`. `<version>` is the crate version from `Cargo.toml`; the hash is the abbreviated commit the build script captured; the date is that commit's author date as `YYYY-MM-DD`. A build made outside a Git checkout prints `recon 0.1.0` with no parenthetical rather than a placeholder. Nothing else is printed. The full commit hash, the target triple, and the executable path are reported by `recon context`, not here.
- `-v` and `-V` are not accepted and produce a usage error.

### The command registry

- One registry value holds every command. Each entry declares: the command path; a one-line summary; a longer description; its positional arguments and options, each with a value name, a type, whether it is required, its default, and its constraints; the effects it may have; the permissions it may need; the hosts that support it; the Rust type whose serialization is its success output; and the error codes it can produce.
- The argument parser is constructed from the registry. There is no second place where a command, an option, or a constraint is written, and a command absent from the registry cannot be invoked.
- Help is rendered from the registry. `recon schema` is serialized from the registry. Argument and option facts in the schema are read back from the constructed parser rather than from the declaration, so that what is advertised is what is enforced.
- The parser is built with clap 4. Recon takes clap's parsing, help rendering, and error introspection, and takes none of its output behaviour: clap's own help and version flags, its error rendering, its exit calls, and its colour are all disabled, and Recon prints help, version, and errors itself.
- The strictness rules are contract, not accident. Options may appear before or after the target. A repeated option is an error. An unknown option is an error. An option whose value is missing, or whose value is another option, is an error. An extra positional argument is an error. A number is written as plain decimal digits with no sign, space, separator, radix prefix, or fractional part, so `+12`, ` 12`, `12.0`, and `0x10` are all rejected. `--pid` must be greater than zero. Clap enforces repetition, unknown options, and positional cardinality. Numeric and output-path options route hyphen-prefixed tokens, including attached values, through their typed value parsers so rejection identifies the option that needs a value. The numeric parser tests for ASCII digits before parsing and applies a lower bound of 1 to `--pid`; the output-path parser rejects a leading hyphen.

### `recon schema`

- `recon schema` with no argument prints the index: `{"schemaVersion":1,"recon":"<version>","commands":[{"path":["app","list"],"summary":string,"effects":[string,…],"hosts":[string,…]},…],"errors":{"schema":<JSON Schema>,"codes":[{"code":string,"exitCode":integer,"retryable":boolean,"summary":string},…]}}`. Commands are listed in a stable order: `app`, then `ui`, then the meta-commands, each group in the order its operations are defined.
- `recon schema <group> <operation>` and `recon schema <meta-command>` print one command's full specification: `{"schemaVersion":1,"recon":"<version>","command":{"path":[string,…],"summary":string,"description":string,"arguments":[…],"options":[…],"effects":[…],"permissions":[…],"hosts":[…],"output":<JSON Schema>,"errors":[string,…]}}`.
    - An argument is `{"name":string,"valueName":string,"type":string,"required":boolean,"description":string,"constraints":object|null}`.
    - An option is `{"name":"--pid","valueName":string,"type":string,"required":boolean,"default":value|null,"repeatable":false,"description":string,"constraints":object|null}`. `constraints` holds the machine-checkable restrictions on the value, such as `{"pattern":"^[0-9]+$","minimum":1}`.
    - `effects` is the set of changes the command may make outside its own process, drawn from `activates-application` and `writes-file`. An empty array means the command is read-only. It is an array rather than a single class because `ui screenshot` writes a file while reading, and a later command may do more than one thing.
    - `permissions` is `[{"permission":string,"condition":string|null}]`, where `permission` is `screen-recording` or `accessibility`, and `condition` is `null` when the permission is always needed and otherwise a short human-readable statement of when it is. The list names every permission the command may need; which of them are held is what `recon context` answers. The conditions are not machine-evaluable, and the schema says so in its description.
    - `hosts` lists the host operating systems that support the command, from `macos`, or the single member `any` when every host does. `any` never appears alongside another member.
    - `output` is a JSON Schema whose `$schema` is `https://json-schema.org/draft/2020-12/schema`, describing the one object the command prints to standard output on success. It is self-contained: any `$ref` it uses points inside its own `$defs`, and no schema fetches anything over a network.
    - `errors` lists the codes this command can produce, as a subset of the codes in the index.
- `schema` and `context` are themselves registered commands, so `recon schema schema` and `recon schema context` describe them, and the index lists them.
- `recon schema` with a path that names no command is a usage error naming the path and listing the accepted command paths.
- `recon schema` takes no options, reads nothing outside Recon, needs no permission, and works on every host.

### Effects, permissions, and hosts of the existing commands

| Command | Effects | Permissions | Hosts |
| --- | --- | --- | --- |
| `app list` | — | — | macOS |
| `app inspect` | — | — | macOS |
| `app launch` | `activates-application` | — | macOS |
| `ui windows` | — | Screen Recording | macOS |
| `ui tree` | — | Accessibility | macOS |
| `ui screenshot` | `writes-file` | Screen Recording always; Accessibility when `--window` is absent | macOS |
| `schema` | — | — | any |
| `context` | — | — | any |

`context` re-executes itself, which is not an effect on the host and is recorded in its description rather than in `effects`.

### Errors

- Every failure prints exactly one JSON object to standard error and nothing else, and prints nothing to standard output.
- The object is `{"error":{"code":string,"message":string,"retryable":boolean,"command":[string,…]|null,"details":object|null}}`.
    - `code` is one of the codes below. The set is closed; a failure Recon did not anticipate is `INTERNAL`, never an undeclared code.
    - `message` is one human-readable sentence. Where a command already has wording today, that wording is the message.
    - `retryable` is `true` when repeating the identical command, unchanged, could succeed because the cause is a transient condition of the system, and `false` when something must change first — the arguments, the state of the target application, or a permission a person must grant.
    - `command` is the resolved command path, or `null` when the arguments named no command.
    - `details` carries the data needed to recover, as structured values rather than prose. It is `null` only when there is nothing to carry.
- The object is printed compactly on a single line followed by a newline. There is no indentation, prose prefix, or second diagnostic on standard error. Its shape is published in `recon schema`'s `errors.schema`.
- Exit status is 0 on success, 2 for a failure Recon can determine from the command line alone without consulting macOS or the filesystem, and 1 for every other failure. Recon never exits 126 or 127; POSIX reserves those for the shell. A command stopped by a signal is unchanged by this proposal: the re-executed child's termination is re-raised by its parent as decision 0001 requires, so the shell reports a status greater than 128.
- The re-executing parent owns the final response. Before the child exists, the parent reports its own failures in exactly this form and exits 1. After spawning, it withholds child standard output and standard error while draining both streams concurrently and waiting for the child. After a successful wait, it forwards the child's bytes unchanged on their respective streams and preserves the child's ordinary exit status, without an additional parent summary for a nonzero child. Signal forwarding and re-raising remain unchanged.
- On a genuine parent wait failure, the parent discards both child streams and emits only one compact, newline-terminated `IDENTITY_UNAVAILABLE` error with `details.stage: "wait"`, exits 1, and leaves standard output empty. This also applies to `context`: no parent fallback success report is produced after a child has been spawned and the wait fails. Cleanup must not blindly signal a process ID that may have been reused after `ECHILD`.
- Both child streams must be drained concurrently, including output larger than a pipe buffer. Waiting before draining, or draining a full stream sequentially before the other, can deadlock. This ownership change introduces no dependencies, retries, output caps, or public modes.

### Error codes

`INVALID_ARGUMENT` is the only code that exits 2. Every other code exits 1.

| Code | Retryable | Raised by | `details` |
| --- | --- | --- | --- |
| `INVALID_ARGUMENT` | no | An argument that is not UTF-8; an unknown command word; a group with no operation; a missing target; an extra positional; an unknown option; an option the command does not take; a repeated option; a missing or hyphen-prefixed option value; a number that is not plain digits; `--pid 0`; a missing required option such as `--out`; a `recon schema` path that names no command | `{"argument":string\|null,"value":string\|null,"expected":string,"accepted":[string,…]\|null}` |
| `UNSUPPORTED_HOST` | no | An `app` or `ui` command on a host other than macOS | `{"host":string,"supportedHosts":["macos"]}` |
| `DISCOVERY_UNAVAILABLE` | yes | `mdfind` could not be run, exited unsuccessfully, or reported no indexed applications | `{"mechanism":"spotlight","detail":string\|null}` |
| `NOT_FOUND` | no | No application matches the selector; a path that cannot be read; `--pid` naming a process that is not an instance of the bundle; `--window` naming a window the instance does not own; a window with no reachable accessibility element; an application reporting no main window | `{"kind":"application"\|"path"\|"instance"\|"window"\|"window-element"\|"main-window","selector":string\|null,"path":string\|null,"pid":integer\|null,"windowId":integer\|null,"runningPids":[integer,…]\|null,"detail":string\|null}` |
| `NOT_AN_APPLICATION` | no | A path that exists but is not an application bundle, whose `Info.plist` cannot be read as a dictionary, or that yields no name. Package type is not a rejection criterion, as corrected in PR #29 | `{"path":string,"reason":"not-a-bundle-directory"\|"unreadable-info-plist"\|"no-name","detail":string\|null}` |
| `AMBIGUOUS_TARGET` | no | A name or identifier matching more than one bundle | `{"selector":string,"candidates":[{"name":string,"bundleId":string\|null,"path":string},…]}` |
| `AMBIGUOUS_INSTANCE` | no | A bundle with several running processes and no `--pid` | `{"application":string,"pids":[integer,…]}` |
| `NOT_RUNNING` | no | A `ui` command against a bundle with no running process | `{"application":string}` |
| `PERMISSION_DENIED` | no | A missing Screen Recording or Accessibility grant | `{"permissions":[{"permission":string,"settingsPane":string},…],"executablePath":string,"adHocSigned":boolean}` |
| `IDENTITY_UNAVAILABLE` | no | Recon cannot become responsible for its own privacy requests: a missing responsibility symbol, an unreadable executable path, an argument containing a null byte, a failed spawn or disclaim, a failed wait, or a child that is still not responsible | `{"stage":"lookup"\|"spawn"\|"disclaim"\|"wait"\|"verify","symbol":string\|null,"detail":string\|null}` |
| `INTERFACE_UNAVAILABLE` | no | A private interface Recon needs is absent on this macOS version: SkyLight capture, the window-server connection, accessibility window-id lookup, or remote-token lookup | `{"interface":string,"symbol":string}` |
| `TIMEOUT` | yes | An application did not answer an accessibility request within the messaging timeout | `{"pid":integer,"timeoutSeconds":2}` |
| `CAPTURE_FAILED` | yes | macOS returned no image, no pixels, an unsupported pixel format, or an image too large to report, or the PNG could not be encoded | `{"windowId":integer,"stage":"capture"\|"pixels"\|"encode","detail":string\|null}` |
| `LAUNCH_FAILED` | no | A bundle with no executable file; a bundle path that is not UTF-8; macOS refusing to open the application, opening it without a process ID, or not opening it; a failed launch callback | `{"application":string,"reason":"executable-missing"\|"path-not-utf8"\|"refused"\|"no-process-id"\|"not-opened"\|"callback-failed","detail":string\|null}` |
| `ALREADY_EXISTS` | no | `--out` names a file that already exists | `{"path":string}` |
| `IO_FAILED` | no | `--out` names no file or a parent directory that does not exist; writing the PNG failed; the current directory could not be read. When a partial file was written and macOS refused to remove it, `partialFile` names it | `{"path":string\|null,"operation":"resolve-output"\|"write"\|"read-current-directory","reason":string\|null,"detail":string,"partialFile":string\|null}` |
| `INTERNAL` | no | A response could not be encoded, a process ID exceeded the range macOS accepts, or any failure Recon did not anticipate | `{"detail":string}` |

Two existing messages are split rather than carried over verbatim, because each conflates two different recoveries:

- `window <id> has no accessibility element reachable by Recon (remote-token lookup is unavailable on this macOS version)` becomes `INTERFACE_UNAVAILABLE` when the remote-token symbol is absent and `NOT_FOUND` with `kind: "window-element"` when the symbol exists and the scan found nothing. An agent can act on the second and not on the first.
- `not an application: <path>` keeps its wording as the message but carries `reason`, so an unreadable `Info.plist` is distinguishable from a directory that is not a bundle.

### Usage errors

- A usage error names the argument at fault and what was expected, and lists the accepted values when the set is finite.
    - `recon ui tree Calculator --window abc` → `--window expects a window id written as decimal digits; got "abc"`, with `{"argument":"--window","value":"abc","expected":"a window id written as decimal digits","accepted":null}`.
    - `recon ui screenshot Calculator` → `recon ui screenshot requires --out`, with `{"argument":"--out","value":null,"expected":"a path to a PNG file that does not exist yet","accepted":null}`.
    - `recon ui windows Calculator --bogus` → `recon ui windows does not take --bogus`, with `{"argument":"--bogus","value":null,"expected":"an option recon ui windows accepts","accepted":["--pid"]}`.
    - `recon app list --json` → `recon app list does not take --json`, with `{"argument":"--json","value":null,"expected":"an option recon app list accepts","accepted":[]}`.
    - `recon app` → `recon app requires an operation`, with `{"argument":null,"value":null,"expected":"an operation of recon app","accepted":["list","inspect","launch"]}`.
    - `recon app inspect Safari Mail` → `recon app inspect takes one target; "Mail" is extra`, with `{"argument":null,"value":"Mail","expected":"exactly one target","accepted":null}`.
    - `recon ui tree Calculator --pid 1 --pid 2` → `--pid was given more than once`, with `{"argument":"--pid","value":"2","expected":"--pid at most once","accepted":null}`.
    - `recon ui tree Calculator --pid 0` → `--pid expects a process id greater than 0; got "0"`.
- `recon` with no arguments prints help and exits 0; a group with no operation is a usage error, because an agent recovers faster from `accepted: ["list","inspect","launch"]` than from a page of help.

### `recon context`

- `recon context` prints one JSON object to standard output and exits 0. It takes no options and no arguments. It reads state; it changes nothing, requests nothing, and can show no dialog.
- Before reading anything, `recon context` makes Recon responsible for itself exactly as a `ui` command does, so that the permission state it reports is Recon's own rather than its launcher's. An identity failure before this process spawns a child does not fail `context`: the report is produced anyway, says that Recon is not responsible for itself, and says why. A genuine wait failure after spawning is different: the parent discards any child output and reports the single `IDENTITY_UNAVAILABLE` failure described above, without a fallback success report.
- Permissions are read with preflight checks only: `CGPreflightScreenCaptureAccess` for Screen Recording and `AXIsProcessTrusted` for Accessibility. `CGRequestScreenCaptureAccess` and `AXIsProcessTrustedWithOptions` are never called from this command.
- Output: `{"recon":{"version":string,"commit":string|null,"executablePath":string},"host":{"os":string,"version":string|null,"architecture":string},"macos":{…}|null}`.
    - `recon.version` is the crate version, `recon.commit` the full commit hash of the build or `null`, and `recon.executablePath` the canonicalized absolute path of the running executable — the path a macOS grant is recorded against.
    - `host.os` and `host.architecture` are the compiled-in target operating system and architecture. `host.version` is the macOS product version read from `ProcessInfo.operatingSystemVersion` as `major.minor.patch`, and `null` on other hosts.
    - `macos` is `null` on hosts other than macOS and otherwise an object with the five members below.
- `macos.codeSigning` is `{"signed":boolean,"identifier":string|null,"teamIdentifier":string|null,"adHoc":boolean}`, read from `SecCodeCopySelf` and `SecCodeCopySigningInformation` requesting `kSecCSSigningInformation`. `signed` is whether `kSecCodeInfoIdentifier` is present; `identifier` and `teamIdentifier` are `kSecCodeInfoIdentifier` and `kSecCodeInfoTeamIdentifier`, each `null` when absent; `adHoc` is whether `kSecCodeInfoFlags` has `kSecCodeSignatureAdhoc` (`0x0002`) set.
- `macos.responsibility` is `{"responsibleForSelf":boolean,"detail":string|null}`. `detail` is `null` when Recon is responsible for itself, and otherwise the reason the self-disclaim did not happen.
- `macos.permissions` is `[{"permission":"screen-recording"|"accessibility","granted":boolean|null,"settingsPane":string},…]`, one entry per permission Recon can need, in that order. `granted` is `null` — never `false` — whenever `responsibleForSelf` is `false`, because a preflight run under the launcher's identity answers a different question, and a `ui` command would fail outright in that state. `settingsPane` is the System Settings location that grants it, the same text the `PERMISSION_DENIED` error carries.
- `macos.interfaces` is `[{"interface":string,"symbol":string,"available":boolean},…]` covering every private interface Recon depends on: `responsibility-query` (`responsibility_get_pid_responsible_for_pid`), `responsibility-disclaim` (`responsibility_spawnattrs_setdisclaim`), `window-server-connection` (`CGSMainConnectionID`), `skylight-capture` (`SLSHWCaptureWindowList`), `accessibility-window-id` (`_AXUIElementGetWindow`), and `accessibility-remote-token` (`_AXUIElementCreateWithRemoteToken`). Availability is decided by looking the symbol up, exactly as the rest of Recon resolves it. Looking a symbol up does not call it and has no effect on any application.
- `macos.applicationDiscovery` is `{"mechanism":"spotlight","available":boolean,"indexedApplications":integer|null}`, measured by running the same `mdfind` query `app list` runs, counting rather than listing. `available` is `false` when the query cannot run, exits unsuccessfully, or reports nothing, which are the three conditions under which `app list` fails. It reports whether Recon's own discovery path works, not whether Spotlight is enabled system-wide; macOS documents no API that answers the latter, and the schema's description says so.
- `recon context` fails only with `INVALID_ARGUMENT`, `INTERNAL`, or `IDENTITY_UNAVAILABLE` for a genuine post-spawn wait failure. A missing permission, a missing private interface, an unsigned or ad-hoc-signed binary, and an identity failure before spawning are reported in the object with exit 0, because reporting them is the command succeeding. Its advertised error list includes the post-spawn wait failure.
- On a host other than macOS, `recon context` succeeds with `macos` set to `null`. It does not report `UNSUPPORTED_HOST`: knowing that this host supports nothing is the answer, not a failure.

### Keeping the schema true

- A command's output schema is generated by `schemars` from the same Rust type whose `Serialize` implementation produces the output, so renaming a field changes both at once.
- Output schemas are generated under schemars' serialize contract, because they describe what Recon prints rather than what it would accept. Under that contract, `ElementNode`'s `valueTruncated` and `childrenOmitted`, which `skip_serializing_if` omits, are optional members rather than required ones. `Node` has a hand-written `Serialize` implementation, so generation alone would lie about it; it gets a hand-written `JsonSchema` implementation beside it in the same file.
- Generated output schemas are closed: every object Recon fully controls sets `additionalProperties: false`. A field added to a serialized type without reaching the schema then fails validation instead of passing silently.
- Tests enforce the invariant rather than restating it:
    - For every command, a real success output — recorded from the CI-exercisable commands and from the macOS integration tests for the permission-gated ones — validates against that command's advertised `output` schema, with a JSON Schema 2020-12 validator.
    - For `ui tree`, the validated outputs include a tree containing a repeated node, a failed node, a truncated value, and omitted children, so the hand-written `Node` schema cannot drift from the hand-written serializer.
    - Every error Recon produces in the test suite validates against `errors.schema`, and its code appears in the producing command's advertised `errors` list.
    - Every code in the enumeration appears in at least one command's `errors` list, and no command advertises a code outside the enumeration.
    - Every registry entry is reachable through the parser, and `recon schema <path>` succeeds for every path the index lists.
- These are tests in the crate, run by `vp run rust`, which is Tier 0 of `.agents/rules/enforcement-hierarchy.md`. The structural half of the rule — a command cannot exist outside the registry, and a failure cannot exist without a code — is enforced by construction: the parser is built from the registry, and the failure type has no constructor that does not take a code.

### A policy for later commands

Mark's approval of this proposal authorizes recording this policy at completion: **every Recon command is defined by a registry entry, and every failure Recon reports carries a code from the published enumeration**. The policy exists because the enforcement is partly structural and partly a test, and a future contributor needs to know which rule those mechanisms are protecting; the mechanisms themselves, not the prose, are what keep it true. Adding an error code is an ordinary change; removing or renaming one changes the published contract.

## Compatibility

Success output is unchanged. Every command that succeeds today prints the same compact JSON object to standard output and exits 0.

Failure output changes shape. `recon: <message>` on standard error becomes one compact JSON object on a single line whose `message` is, in nearly every case, the same sentence. Anything matching on the old text breaks; nothing is known to.

Exit codes change. A usage error now exits 2 rather than 1. Everything that exits nonzero still exits nonzero, so a caller that only checks success or failure is unaffected.

Orientation changes from failure to success. `recon`, `recon --help`, `recon -h`, `recon help`, and `recon --version` printed a usage block to standard error and exited 1; they now print help or a version to standard output and exit 0. `recon app` and `recon ui` printed the same block and now produce a usage error naming the missing operation, still on standard error, now exiting 2 rather than 1.

Two existing error messages are split, as described in **Error codes**. Both keep wording that is true of the narrower case.

Re-executed output is now withheld until the parent successfully waits for the child, rather than flowing directly through inherited output streams. Ordinary child success and failure bytes and exit status are unchanged after a successful wait. A genuine wait failure discards any child response and becomes one parent `IDENTITY_UNAVAILABLE` error with empty standard output, including for `context`; the earlier fallback behavior applies only before spawning. Standard input inheritance and signal forwarding/re-raising are unchanged.

Both guides change. `guides/macos-application-investigation.md` describes failures as "a diagnostic on standard error, a nonzero exit status, and no success JSON" and names the `application executable is missing` wording. `guides/macos-ui-observation.md` quotes the permission error block, the instance and window error sentences, the capture and `--out` failures, and the non-macOS message, and states that every failure produces "a diagnostic on standard error". Each passage is rewritten against the error object, and both guides gain the orientation, schema, and context commands.

Recon has no released version, no package, and no users outside this repository, so no migration path is needed. Recon stores no data.

## Implications on adoption

- The CLI gains dependencies: `clap` for parsing and help, `schemars` for output schemas, `objc2-security` for reading Recon's own signing information, and a JSON Schema validator as a development dependency. All are ordinary registry crates; none changes how Recon is installed or signed.
- The build script gains the commit hash and date for `--version` and `recon context`, and must rebuild when `HEAD` moves. A build from a source archive with no Git metadata keeps working and reports no commit.
- Decision 0001 must be superseded as described in **Policies and decisions checked** before `recon context` can be implemented as specified.
- Nothing about granting, signing, or installing Recon changes. An existing grant stays valid; `recon context` reads it and never asks for it.
- Adoption is reversible in the sense that matters here: no stored state, no configuration, nothing a user must undo.

## Scope

- The command registry, the parser built from it, help rendering, and `recon --version`, covering all six existing commands.
- The structured error object, the code enumeration, the exit-code scheme, and the conversion of every failure path in the crate — including those raised by the re-executing parent — onto it.
- `recon schema`, with the index, per-command specifications, and output schemas generated from the serialized types.
- `recon context`, including the self-disclaiming re-execution with its non-fatal fallback, preflight-only permission checks, code-signing facts, host facts, private-interface availability, and application-discovery availability.
- A decision superseding decision 0001 to cover `recon context`'s re-execution and preflight-only checks, preserving the original decision's rationale and history.
- Tests: the strictness rules as behaviour tests against the new parser; help and version output; the schema index and every per-command specification; validation of real outputs and real errors against the advertised schemas; the code-coverage and registry-coverage checks described in **Keeping the schema true**; and macOS integration tests for `recon context` under the existing opt-in gate.
- Updates to `guides/macos-application-investigation.md` and `guides/macos-ui-observation.md`, and a short guide section covering orientation, `recon schema`, `recon context`, and how to read an error object.
- The policy recorded at completion under Mark's approval of this proposal.

### Out of scope

- Options and output shapes that narrow what `app list` and `ui tree` return. Those are a separate proposal, <https://github.com/markmals/recon/pull/35>. This proposal defines the registry and the error contract generically and specifies no narrowing option.
- `cli search` over the registry. Natural-language command discovery is worth building when Recon has dozens of commands, not six; the registry this proposal creates is its prerequisite.
- A `docs search` command and an agent skill for consumers of the CLI. Both are documentation delivery, not interface contract.
- An MCP server. The guidance is explicit that MCP tools should be generated from the same registry; that is a later use of this work, not part of it.
- Renaming or restructuring any existing command. The grammar rule is written down so later commands are consistent; existing names encode real macOS distinctions and stay.
- `--dry-run`, confirmation flags, and receipts. Recon's only command with an effect on an application is `launch`, and `app inspect` already previews it. These matter for interaction (#24) and experiments (#18).
- Any change to what Recon observes or how it observes it.
- Windows adapters and the evidence store.

## Preview

- Artifact: the existing `macos-preview` CI artifact for this pull request, `recon-macos-arm64`, containing the ad-hoc-signed `recon` executable and the built fixture, as in proposals 0001 and 0002.

    The reviewer runs `recon --help`, `recon ui tree --help`, and `recon --version`, and checks that each prints to standard output and exits 0. They run `recon schema` and `recon schema ui tree`, and validate a real `recon ui windows` result against the advertised output schema with any JSON Schema validator. They run `recon context` on the unsigned CI build and check that it reports `adHoc: true`, an empty or absent team identifier, whichever permissions this Mac has granted to that path, and every private interface as available — without any dialog appearing. They provoke a usage error with `recon ui tree Calculator --window abc` and check for exit 2 and a `INVALID_ARGUMENT` object naming `--window`, and a runtime failure with `recon ui tree NoSuchApplication` and check for exit 1 and `NOT_FOUND`. Finally they re-sign the executable with their Developer ID identity as the guide describes and re-run `recon context` to see `adHoc: false` and the identifier and team.

- Reason: the contract is observable only by running the binary, and the interesting cases — an ad-hoc signature, a missing permission reported without a prompt, a private interface present or absent — exist only on a real Mac. The artifact already exists and needs no new CI mechanism. Rendered documentation cannot exercise any of it.

## Policies and decisions checked

- `policies/` contains only a README; no policy applies. Mark approved recording the first one at completion, as described in **A policy for later commands**.
- `decisions/0001-macos-privacy-identity.md` is honored except for the approved extension below, which requires a superseding decision rather than rewriting the original decision's meaning.
    - Honored: Recon's privacy identifier is unchanged; `recon context` makes Recon responsible for itself before reading permission state, so a grant still means the same thing under every launcher; Recon never writes to the TCC database or works around a missing grant; and the `PERMISSION_DENIED` error still names the permission, the executable's path, and the System Settings pane, now as structured fields.
    - Conflict: the decision says "Commands that need no permission do not re-execute", and `recon context` needs no permission yet must re-execute, because a preflight check run under the launcher's identity reports the launcher's grants. The decision also says Recon "checks a permission without prompting, asks macOS to request it when it is missing, and then fails"; `recon context` checks without prompting and then neither requests nor fails.
    - Approved superseding decision: record a new decision established by this proposal and superseding decision 0001, preserving the original rationale. A command re-executes when its result depends on Recon's own privacy identity, whether because it needs a permission or because it reports Recon's permission state. Other commands do not re-execute. `context` preflights permissions without requesting them and reports unknown grants on an identity failure before spawning. The parent owns and withholds both child output streams until a successful wait, then forwards their bytes and preserves status and signal semantics; on a genuine wait failure it discards child output and emits only `IDENTITY_UNAVAILABLE` with stage `wait`, with no `context` fallback report.
- `VISION.md` is served by version, context, and error contracts that an agent can parse and trace to their conditions. Human-readable `message` sentences preserve legibility within the compact single-line JSON error format Mark selected.

## Future directions

- A `cli search` command over the registry becomes worthwhile once the reverse-engineering command families (#2–#14) land and the catalogue is too large to list.
- MCP tool definitions could be generated from the same registry, as could shell completions, which `clap_complete` produces from the parser this proposal builds.
- An agent skill for consumers of the CLI could direct an agent to `recon schema` and `recon context` rather than duplicating a reference manual.
- Interaction (#24) and experiments (#18) could add `--dry-run` and receipts, and would declare their effects in the same registry field this proposal defines.
- The evidence store (#23) could record a `recon context` report alongside each observation, so a stored claim carries the version, identity, permissions, and host that produced it.
- Experiment files (#18) could reference a `$schema` served by the installed binary rather than a remote branch file, removing the drift between a schema URL and the binary reading it.

## Alternatives considered

- **A hand-written registry and parser, with no clap.** Recon's parser is 99 lines and already enforces every strictness rule, and avoiding clap would keep the dependency tree small for a binary whose supply chain is part of its evidence. Rejected because the grounding showed clap's defaults preserve five of the seven rules outright and the sixth is a three-line value parser; because clap's structured error context removes the need to invent one; and because a hand-written help renderer and schema walker is work clap and `clap_complete` have already done and tested. The reason to prefer hand-writing would be clap getting in the way of strictness, and it does not.
- **clap's derive macros rather than a builder-based registry.** Rejected because the registry must carry metadata clap has no field for — effects, permissions, hosts, output type, error codes — and attaching that to derive attributes would put half of each command's description in a macro and half elsewhere. The builder takes the registry entry as a value and returns a `Command`.
- **Prose errors by default, with structured errors behind `--error-format json`.** Rejected because an agent with no prior knowledge of Recon cannot opt into a flag it does not know exists, which is the problem this proposal addresses. Every caller receives the same structured error format.
- **A prose line followed by the JSON object on standard error.** Rejected: it gives a parser two things to find on one stream and a reader the same sentence twice.
- **Indented error objects.** Rejected in favor of compact single-line JSON, as Mark selected on 2026-10-26. The human-readable `message` remains; callers can format the JSON for display without requiring a second output mode.
- **`sysexits.h` codes — 64 for usage, 77 for permission denied, 69 for unavailable.** Rejected. Its two primary sources contradict each other, clap considered and declined it for the same reason, and a code table that varies by platform is worse than one number plus a stable string code that carries the real distinction.
- **A distinct exit code per error class.** Rejected: the exit code answers "did it work and was it my command line", and `code` answers everything else. Spreading the taxonomy across both means an agent must learn two of them.
- **`recon doctor`.** Rejected. "Doctor" implies diagnosis and repair, and this command must never repair anything — in particular it must never request a permission. The guidance's vocabulary has no `doctor`.
- **`recon status`.** Rejected. The guidance defines `status` as "Retrieve operational state" of a resource, which in Recon's grammar would be `recon <resource> status`. A top-level `status` would read as the state of something Recon is running.
- **`recon context show`, following the guidance's `mycli context show` literally.** Rejected. That form exists because `cf` also switches contexts; Recon has one context and nothing to switch, so the operation word would carry no information.
- **Open output schemas, without `additionalProperties: false`.** Rejected: an open schema cannot detect the drift the invariant exists to prevent.
- **Publishing the schema as a static JSON file in the repository.** Rejected. The question an agent asks is what _this binary_ does, and a file in a branch can disagree with the binary in hand — the same drift the guidance warns about and that issue #18's remote `$schema` URL would reproduce.
- **A `+` SemVer build-metadata suffix in `--version`, as `0.1.0+a3731c8`.** Rejected in favour of the parenthetical form `cargo` and `rustc` use, which is what readers of Rust tooling expect and which keeps the version number itself the last token before a space, as the GNU standards ask.
- **A bare `recon <version>` with no commit.** Rejected. Recon will sit at `0.1.0` across many behaviour changes, and `VISION.md` requires a claim to be traceable to the conditions that produced it; a version that cannot identify the build cannot do that.

## Open questions

None. Mark resolved the default error format on 2026-10-26: always one compact JSON error object on a single line.

## Acknowledgments

The agent-first CLI guidance that prompted this work, and Cloudflare's published experience building `cf`, supplied the discovery, schema, and error contracts this proposal adapts to a local macOS tool.
