Stargate (Rust implementation of Dreamland):
--------------------------------------------
- [UNIX userland was always as mess, you're just used to it](https://www.linkedin.com/pulse/unix-userland-always-mess-youre-just-used-dmitry-kalashnikov-2k6sc)
- ever wondered why its rm -rf, yet its chown -Rf dvk:dvk? ls ("list" what? I think you mean directory files .. etc)
- standardizing UNIX "userland" (commands you type) naming with verb-noun and their parameters (-h always means help, -v verbose and so on). Its obvious that some parameters are common, some unique per command. Needs a thin parameter parsing
 layer. And structured (command) output for selection instead of searching through text streams (super slow, big-O). This is also a common parameter.
- some commands are focused on doing one thing and doing it well, and can be expressed as a verb-noun: ls is list-directory. Other commands (already) handle multiple verbs: hostname (hostname: "set or print name of current host system"). They can be split into set-hostname and get-hostname commands (disk space is not a concern in 2025). Or they need to be noun verb instead of verb noun: freebsd-update fetch (already does that .. that what we want). Another good example: "pkg update". There is going to be a noun and a verb (or vise-versa).
- aliases are two different things: (1) short names for longer commands and (2) their default params: some-long-command is slc. Convention over configuration.
- Rust is infinitely superior to C for implementing a new UNIX userland. C is an ancient procedural language for working with bare metal and kernel -- the userland code requires higher levels of abstraction, memory safety, OO, functional idioms, ability to leverage design patterns, ddd, built-in support for unit testing, internationalization, etc. Also rust has better error handling, support for modules, its much more expressive, enums and patterns, traits and generics, closures, iterators, collections, infinitely better strings and text handling, concurrency, async idioms, macros, etc.

## Non-Goals
- supporting UNIX POSIX compatibility (legacy ways of interacting with UNIX through a command-line interface).
- supporting Windows compatibility (just use Windows Powershell instead). Its kind of ridiculous that every command in (rust) coreutils was handling how Windows works. No one that runs Windows cares about coreutils.
- supporting SELinux and Android.

## Goals
- reduce it to OpenBSD-style crystalized essentials. The BSD userland (compared to GNU/CoreUtils, including rust rewrite) is much much smaller (by 10x if its OpenBSD).
- Build from there: split all commands. So they do one thing and do it well. Right now they're doing a lot of things. Renaming all commands to verb-noun to reduce mental friction. So it sounds just like English.
- Standardize on input parameters (don't care about legacy POSIX): -r is always recursive, -v is always verbose ... etc. Stuff works as expected.
- Introduce a super thin object layer (for optional object output), so piping is faster by an order of magnitude (in some cases). Instead of searching in unstructured streams of text output (such as stdout), it will be a selection, slicing and dicing. Monad / MS Powershell has awesome design ideas. They came from UNIX ideas. Some UNIX ideas: Do one thing and do it well. Everything is a file (including pipes, stdout (just a special file), stderr (its also just a special file), directories are files, pipes and sockets are also files, etc).
