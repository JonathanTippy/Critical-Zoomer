#[derive(Copy, Clone)]
pub struct Highlights {
    pub in_filament: bool
    , pub out_filament: bool
    , pub small_time_edge: bool
    , pub node: bool
}

impl Highlights {
    pub fn new() -> Self {
        Highlights {
            in_filament: false
            , out_filament: false
            , small_time_edge: false
            , node: false
        }
    }
}