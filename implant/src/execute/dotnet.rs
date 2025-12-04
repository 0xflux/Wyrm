pub fn execute_dotnet() {
    /**
     * SCRATCHPAD
     */
    
    // NOTE: This fn could be turned into a sub-DLL or something we can inject into a sacrificial process if
    // we want it loaded in a foreign process?

    // For local dev, from a stack overflow post:
    // 1) Load  dotnet file into a string
    // 2) Copy file data into a SAFEARRAY
    // 3) Load managed assembly
    // 4) Get entrypoint of the assembly
    // 5) Get params of entrypoint and save in SAFEARRAY
    // 6) Call entrypoint, passing in params


    /**
     * CODE SECTION
     */

}
