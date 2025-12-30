# Static WOFs

WOFs, aka Wyrm Object Files, are includable libraries (supported are C & C++ with Rust coming soon). These
allow you to write your own source code, or to allow you to include existing tooling, 
to perform certain routines in a supported language of your choice.

For compile time (aka static) WOFs, you do not need to make them position independent. Currently, there is no interaction with 
the Wyrm agent through a WOF, but this could come in the future if there is demand.

Simply, drop your project inside of `wofs_static` and in your profile, simply specify the folder name of the project,
for example if you drop Mimikatz source code inside the folder name `mimikatz`, you would enter:

```toml
wofs = ["mimikatz"]
```

If you have a second, third, fourth project to link, you can:

```toml
wofs = ["mimikatz", "second", "third", "fourth"]
```

These will be statically linked into Wyrm at compile time. You can then get them to run through the wof command on the c2,
by entering wof followed by the folder name to which you stored them. Using the above example: `wof mimikatz`.