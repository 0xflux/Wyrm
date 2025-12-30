# Static WOFs

WOFs (Wyrm Object Files) are small, self-contained code modules that are baked into the implant at compile time.
They're intended for pulling in existing tooling (e.g. Mimikatz, custom helpers) or for writing one-off 
routines in C/C++ (and pre-built Rust/Zig object files).

Static WOFs are not DLLs and do not need to be position-independent; they are compiled and linked directly into 
the Wyrm implant as normal object files.

At the moment there is no formal 'Wyrm API' exposed to WOFs beyond a simple FFI entrypoint. They just run as regular 
code inside the process. A richer API can be added later if there is demand for it.

**Note**: If you wish anything to be printed to the terminal and to have that visible in the C2, you must write to
`STD_OUTPUT_HANDLE`. See an example below. **Warning**: Failing to do this correctly could result in output going to the
(hidden) console window of the agent.

## Where WOFs live

All static WOFs are placed under the `wofs_static` directory in the repository. Each top level subdirectory under `wofs_static` 
is treated as a separate WOF module.

### Example layout:

```
wofs_static/
    1/
        main_inc.c
        main_inc.h
        main.c
    2/
        main.c
        print_fn.c
        sub/
            my_header.h
    3/
        rust.o
    Readme.md
```

You can name these folders whatever you like in a real profile:

- mimikatz
- crypto_helpers
- screenshooter
- etc.

The numbers (1, 2, 3) above are just an example.

## Writing a WOF in C/C++

A minimal example in wofs_static/2 might look like:

`sub/my_header.h`

- Defines any shared prototypes.
- Includes `<windows.h>` and any other headers you need.

`print_fn.c`

- Implements helper routines, e.g. write_console(char *msg) that writes to `STD_OUTPUT_HANDLE`.

`main.c`

- Implements the actual WOF entrypoint function that you want Wyrm to call.

You may wish to implement `main.c` as:

```C
#include "sub/my_header.h"

void ffi_two() {
    char* wof_msg = "Hello from WOF\0";
    write_console(wof_msg);

    MessageBoxA(
        0,
        wof_msg,
        wof_msg,
        MB_OK
    );

    return 0;
}
```

And `print_fn.c` as:

```C
#include "sub/my_header.h"

void write_console(char* msg) {
    HANDLE h = GetStdHandle(STD_OUTPUT_HANDLE);
    DWORD written;
    WriteFile(h, msg, (DWORD)strlen(msg), &written, 0);
}
```

And so on..

## Passing arguments to a WOF

Static WOFs can take a single string argument from the C2. From the operator’s point of view, the syntax looks like:

With an argument: `wof my_function "Hello from WOF"`

Without an argument: `wof my_function`

This will allow you to pass some data into your entrypoint - this could be a good way to build a small glue like parser
for another tool - for example, if you wish to bundle tool x, but tool x takes command line arguments, you can 
slightly modify the code to accept some input instead. You can parse this as whatever you like, albeit a string,
or interpret those bytes as another type.

The Wyrm C2 will automatically append a null byte to the end of your input, so please do not worry about doing that
yourself.

Example usage for C (also applicable with Rust, etc):

```C
int my_function(char* msg) {
    printf("%s\n", msg);

    int result = MessageBoxA(
        0,
        msg,
        msg,
        MB_OK
    );

    test(msg);

    return result;
}
```

## Using pre-built objects (e.g. Rust, Zig)

You don't have to use C or C++ directly. You can:

- Compile a Rust (or other language) project to an object file targeting `x86_64-pc-windows-msvc`.
- Drop the resulting `.obj` / `.o` file into a WOF folder under `wofs_static`.

The build script will detect these `.o` / `.obj` files via the same directory walk and treat them as additional object inputs.

### Building in Rust

To build in rust, you want to make sure you are operating in a `no_std` environment and that your crate is a lib,
specifically in your toml:

```toml
[lib]
crate-type = ["staticlib"]
```

Your library then implements your chosen behaviour, and you need at least one linkable symbol (via `pub extern "system" fn`), 
for example:

```rust
#![no_std]
#![no_main]

use core::ptr::null_mut;

use windows_sys::Win32::UI::WindowsAndMessaging::{MB_OK, MessageBoxA};

#[cfg_attr(not(test), panic_handler)]
#[allow(unused)]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
pub extern "system" fn rust_bof() -> u32 {
    let msg = "rust bof\0";
    unsafe {
        MessageBoxA(null_mut(), msg.as_ptr(), msg.as_ptr(), MB_OK);
    }

    0
}
```

Note that you can include external crates as normal; but they should be no-std compliant. If you want to interact
with the Windows API easily, I would recommend the [windows_sys](https://crates.io/crates/windows-sys) crate.

You can then compile this to a .o file:

```shell
cargo rustc --lib --target x86_64-pc-windows-msvc --release -- --emit=obj -C codegen-units=1
```

And now you can move the output `.o` file into `wofs_static` under a directory name for it to link up to your profile 
toml on the C2.

## Wiring WOFs via a profile

From the `C2/profiles` side you don't manually set WOF.
Instead, you configure a list of WOF folders, and the C2 translates that into the appropriate environment variable before compiling the implant.

Example:

- `wofs = ["mimikatz"]`

or:

- `wofs = ["mimikatz", "crypto_helpers", "screenshotter"]`

Each entry corresponds to a folder under wofs_static:

- `wofs_static/mimikatz`
- `wofs_static/crypto_helpers`
- `wofs_static/screenshotter`

These modules are then statically linked into Wyrm at compile time.

## Executing WOFs from the C2

Once compiled into the implant, WOFs can be triggered from the C2 via the `wof` command.

The command takes the module name (i.e. the folder name you configured in the profile). The agent uses its internal 
WOF metadata to resolve and invoke the appropriate entrypoint function from that module.

Example (using the earlier naming):

```shell
wof mimikatz
```

The exact behaviour (which symbol is used as the entrypoint, additional arguments, etc.) is controlled by the implant's WOF 
execution logic, but from the operator's perspective you only need to remember:

- Add your code under `wofs_static/<name>`.
- Reference `<name>` in the profile's wofs list.
- Use `wof <name>` from the C2 to execute it.