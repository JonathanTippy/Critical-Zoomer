use crate::utils::ObjectivePosAndZoom;

pub enum FinishedAnswer {
    Outside {
        big_time: u32
        , small_time: u32
        , smallness: f64
    }
    , Inside {
        small_time: u32
        , loop_period: u32
        , smallness: f64
    }
}

pub struct ZoomerValuesScreen {
    pub values: Vec<FinishedAnswer>
    , pub res: (u32, u32)
    , pub objective_location: ObjectivePosAndZoom
}
