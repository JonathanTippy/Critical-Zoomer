use crate::action::utils::*;
use crate::action::constants::*;



#[derive(Debug)]

pub(crate) enum Serial<T> {
    Stream{value: T}
    , Move{loc: Box<ObjectivePosAndZoom>}
    , Resize{res: Box<(u32, u32)>}
}

#[derive(Clone, Debug)]

pub(crate) struct Frame<T> {
    pub(crate) pixels: Vec<T>
    , pub(crate) res: (u32, u32)
    , pub(crate) size: usize
    , pub(crate) objective_location: ObjectivePosAndZoom
}

impl<T: Default+Copy> Frame<T> {
    pub(crate) fn with_size(size:usize) -> Self {
        Frame {
            pixels: Vec::with_capacity(size)
            , res: DEFAULT_WINDOW_RES
            , size: DEFAULT_FRAME_SIZE
            , objective_location: ObjectivePosAndZoom::default()
        }
    }
}

impl<T: Default + Copy> Default for Frame<T> {
    fn default() -> Self {
        Frame {
            pixels: Vec::from([T::default();DEFAULT_FRAME_SIZE])
            , res: DEFAULT_WINDOW_RES
            , size: DEFAULT_FRAME_SIZE
            , objective_location: ObjectivePosAndZoom::default()
        }
    }
}

#[derive(Clone, Debug)]

pub(crate) struct DoubleBuffer<T> {
    pub(crate) frame: Frame<T>
    , buffer_frame: Frame<T>
}

impl<T: Default + Copy> Default for DoubleBuffer<T> {
    fn default() -> Self {
        let mut returned = DoubleBuffer {
            frame: Frame::default()
            , buffer_frame: Frame::default()
        };
        
        // clear the buffer frame by resetting the double buffer
        returned.push(
            Serial::Move{
                loc: Box::new(ObjectivePosAndZoom::default())
            }
        );
        
        returned
    }
}

impl<T> DoubleBuffer<T> {
    pub(crate) fn push(&mut self, new:Serial<T>) -> bool {
        match new {
            Serial::Stream{value} => {
                self.buffer_frame.pixels.push(value);
                if self.buffer_frame.pixels.len() == self.buffer_frame.size {
                    std::mem::swap(&mut self.frame, &mut self.buffer_frame);
                    self.buffer_frame.pixels.clear(); true
                } else {false}
            }
            Serial::Resize{res} => {
                println!("buffer is being resized. If you are not resizing the window, this is a bug.");
                let res = *res;
                self.buffer_frame.res = res;
                self.buffer_frame.size = (res.0*res.1) as usize;
                self.buffer_frame.pixels = Vec::with_capacity(self.buffer_frame.size);
                true
            }
            Serial::Move{loc} => {
                let loc = *loc;
                self.buffer_frame.objective_location = loc;
                self.buffer_frame.pixels.clear();
                true
            }
        }
    }

    
}