use windows_sys::Win32::System::Com::SAFEARRAYBOUND;

enum DotnetError {
    BoundOverflow,
}

pub fn execute_dotnet() -> Result<(), DotnetError> {
    //
    // SCRATCHPAD
    //

    // NOTE: This fn could be turned into a sub-DLL or something we can inject into a sacrificial process if
    // we want it loaded in a foreign process?

    // https://stackoverflow.com/questions/35670546/invoking-dotnet-assembly-method-from-c-returns-error-cor-e-safearraytypemismat
    // https://stackoverflow.com/questions/335085/hosting-clr-bad-parameters

    // For local dev, from a stack overflow post:
    // 1) Load  dotnet file into a string
    // 2) Copy file data into a SAFEARRAY
    // 3) Load managed assembly
    // 4) Get entrypoint of the assembly
    // 5) Get params of entrypoint and save in SAFEARRAY
    // 6) Call entrypoint, passing in params

    //
    // CODE SECTION
    //

    // Read file
    let f = std::fs::read_to_string(r"C:\Users\flux\git\Rubeus\Rubeus\bin\Release\Rubeus.exe")
        .expect("could not read file");

    // Copy file into a SAFEARRAY
    let bounds = create_safe_array_bounds(f.len())?;
    let sa = SafeArrayCreate;

    Ok(())
}

fn create_safe_array_bounds(len: usize) -> Result<SAFEARRAYBOUND, DotnetError> {
    // Check we aren't going to overflow the buffer
    if len > u32::MAX as usize {
        return Err(DotnetError::BoundOverflow);
    }

    let mut bounds = SAFEARRAYBOUND::default();
    bounds.cElements = len as u32;

    Ok(bounds)
}
