//! A trigonometric approach to detect human behaviour on an endpoint as seen by
//! LummaC2 https://outpost24.com/blog/lummac2-anti-sandbox-technique-trigonometry-human-detection/

use core::f32::math::sqrt;
use std::{
    thread::sleep,
    time::{Duration, Instant},
};

use cgmath::{Deg, InnerSpace, Vector2};
use windows_sys::Win32::{
    Foundation::{FALSE, POINT, TRUE},
    UI::WindowsAndMessaging::GetCursorPos,
};

const MAX_WAIT_TIME_SECONDS: u64 = 5 * 60; // 5 mins

/// This function attempts to detect a sandbox by monitoring mouse movements and
/// checking for behaviour which would not be expected by a human using some trig and
/// euclidean math.
///
/// If the function detects mouse movement within the capture period of > a constant number
/// of px, or greater than 45 degrees between captures, a sandbox is assumed.
///
/// # No return period
/// The function will not return if no mouse movements are captured; up to the max waiting time,
/// [`MAX_WAIT_TIME`].
///
/// The function will not return if 'bad movements' are detected, up to the max waiting time,
/// [`MAX_WAIT_TIME`].
pub fn trig_mouse_movements() {
    const MAX_POINTS_0_IDX: usize = 30;
    let mut points = [POINT::default(); MAX_POINTS_0_IDX];

    const MAX_TRAVEL_DISTANCE: f32 = 500.;
    const MAX_ANGLE: f32 = 45.;

    let timer: (Instant, Duration) = (Instant::now(), Duration::from_secs(MAX_WAIT_TIME_SECONDS));

    //
    // The bread and butter loop which will continue to get mouse movement and check against
    // mouse movements to detect non-human behaviour. If the 5 min period elapses in this, then
    // it will break.
    //
    // If any mouse movement information is 0, it will try again, until there is full movement observed
    // over the time period measured.
    //
    loop {
        let mut bad_point = false;

        //
        // Get the points
        //
        for i in 0..MAX_POINTS_0_IDX {
            get_pos(points.get_mut(i).unwrap(), &timer);
            sleep(Duration::from_millis(10));
        }

        //
        // Check for non human behaviour
        //
        for (i, point) in points.iter().enumerate() {
            // Check the timer
            if timer.0.elapsed() >= timer.1 {
                bad_point = false;
                break;
            }

            let next_point = match points.get(i + 1) {
                Some(p) => p,
                None => break,
            };

            // calculate the euclidean distance between the points
            let first = i32::pow(point.x - next_point.x, 2);
            let second = i32::pow(point.y - next_point.y, 2);

            let distance = sqrt(first as f32 + second as f32);

            // Calculate the angle between the points
            let v1 = Vector2::new(point.x as f32, point.y as f32);
            let v2 = Vector2::new(next_point.x as f32, next_point.y as f32);
            let angle = Deg::from(v1.angle(v2)).0.abs();

            if angle == 0. || distance == 0. {
                bad_point = true;
            }

            //
            // If the angle is > MAX_ANGLE, or the mouse distance travelled is greater than MAX_TRAVEL_DISTANCE px (??)
            // then we want to cause the test to go again.
            //
            if angle > MAX_ANGLE || distance > MAX_TRAVEL_DISTANCE {
                bad_point = true;
            }
        }

        // If we didn't have a bad point, aka no mouse movement, then break
        if !bad_point {
            break;
        }
    }
}

fn get_pos(point: &mut POINT, live_timer: &(Instant, Duration)) {
    loop {
        // If we waited longer than a sandbox will be watching for
        if live_timer.0.elapsed() >= live_timer.1 {
            break;
        }

        if unsafe { GetCursorPos(point) } == TRUE {
            break;
        };

        sleep(Duration::from_millis(200));
    }
}
