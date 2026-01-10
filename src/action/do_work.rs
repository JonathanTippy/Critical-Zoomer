pub(crate) struct WorkContext {
    active_point: Option<PointInProgress>
    , pos: (i32, i32)
    , res: (u32, u32)
    , step: fn((i32, i32), (u32, u32)) -> Option<(i32, i32)>
}

pub(crate) struct Z {
    real: f64
    , imag: f64
    , real_squared: f64
    , imag_squared: f64
}

pub(crate) enum IterationResult {
    Incomplete{}
    , Escaped{}
    , Repeated{}
}

impl Result {
    fn is_incomplete(&self) -> bool {
        match self {
            Incomplete{..}=>{true}
            _=>{false}
        }
    }
}

impl PointInProgress {
    fn iterate(&self, bailout_radius_squared:f64, epsilon_squared:f64) -> IterationResult {

        self.z.real = self.z.real_squared - self.z.imag_squared
            + self.c.0;
        self.z.imag = self.z.real * self.z.imag
            + self.c.1;

        self.z.real_squared = self.z.real * self.z.real;
        self.z.imag_squared = self.z.imag * self.z.imag;

        if self.did_not_escape(bailout_radius_squared) {} else {return IterationResult::Escaped}

        if self.is_not_near(self.landmark, epsilon_squared) {} else {return IterationResult::Repeated}

        IterationResult::Incomplete
    }

    fn did_not_escape(&self, bailout_radius_squared:f64) -> bool {
        let magnitude_squared = self.z.real_squared + self.z.imag_squared;
        return magnitude_squared <= bailout_radius_squared
    }

    fn is_not_near(&self, point: (f64, f64), epsilon_squared: f64) -> bool {
        let difference = (self.z.real-point.0, self.z.imag-point.1);
        let difference_magnitude_squared = difference.0 * difference.0 + difference.1 * difference.1;
        return difference_magnitude_squared > epsilon_squared;
    }

    fn update_landmarks(&self) {


        for i in 0..self.landmarks.len() {

        }
    }
}


pub(crate) struct PointInProgress {
    c: (f64, f64)
    , z: Z
    , iteration_count: u64
    , loop_checkpoint: (f64, f64)
    , min_magnitude_squared: f64
    , approach_checkpoint: (f64, f64)
    , approach_min_magnitude_squared: f64
    , non_approach_iteration_counter: u64
    , skip_counter: u64
    , period: u64
}

impl PointInProgress {
    fn new(c:(f64,f64)) -> Self {
        PointInProgress {
            c
            , z:Z{real:0, imag:0, real_squared:0, imag_squared:0}
            , iteration_count: 0
            , loop_checkpoint: (0.0, 0.0)
            , min_magnitude_squared: f64::INFINITY
            , approach_checkpoint: (0.0, 0.0)
            , approach_min_magnitude_squared: f64::INFINITY
            , non_approach_iteration_counter: 0
            , skip_counter: 1
            , period: 1
        }
    }
}

fn initialize_point(screen_top_left_corner_location: (f64, f64), pixel_side_length: f64, screen_pos: (i32, i32)) -> PointInProgress {
    return PointInProgress::new(
        (
            screen_top_left_corner_location.0 + pixel_side_length * screen_pos.0 as f64
            , screen_top_left_corner_location.1 - pixel_side_length * screen_pos.1 as f64
            )
    )
}

