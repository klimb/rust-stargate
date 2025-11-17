Stargate (Rust implementation of Dreamland):
--------------------------------------------
- [UNIX userland was always as mess, you're just used to it](https://www.linkedin.com/pulse/unix-userland-always-mess-youre-just-used-dmitry-kalashnikov-2k6sc)
- ever wondered why its rm -rf, yet its chown -Rf dvk:dvk? ls ("list" what? I think you mean directory files .. etc)
- standardizing UNIX "userland" (commands you type) naming with verb-noun and their parameters (-h always means help, -v verbose and so on). Its obvious that some parameters are common, some unique per command. Needs a thin parameter parsing
 layer. And structured (command) output for selection instead of searching through text streams (super slow, big-O). This is also a common parameter.
- some commands are focused on doing one thing and doing it well, and can be expressed as a verb-noun: ls is list-directory. Other commands (already) handle multiple verbs: hostname (hostname: "set or print name of current host system"). They can be split into set-hostname and get-hostname commands (disk space is not a concern in 2025). Or they need to be noun verb instead of verb noun: freebsd-update fetch (already does that .. that what we want). Another good example: "pkg update". There is going to be a noun and a verb (or vise-versa).
- aliases are two different things: (1) short names for longer commands and (2) their default params: some-long-command is slc. Convention over configuration.
- Rust is infinitely superior to C for implementing UNIX userland. C is an ancient procedural language for working with bare metal -- this requires higher levels of abstraction, memory safety, OO, functional idioms, ability to leverage design patterns, ddd, built-in support for unit testing, internationalization, etc.

## Non-Goals
- supporting UNIX POSIX compatibility (legacy ways of interacting with UNIX through a command-line interface).
- supporting Windows compatibility (just use Windows Powershell instead). Its kind of ridiculous that every command in (rust) coreutils was handling how Windows works. No one that runs Windows cares about coreutils.
- supporting SELinux and Android.

