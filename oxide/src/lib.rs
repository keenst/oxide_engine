use std::ffi::c_void;
use std::cmp::min;
use std::cmp::max;
use windows::Win32::Graphics::Gdi::BITMAPINFO;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct OffscreenBuffer {
    pub info: BITMAPINFO,
    pub memory: *mut c_void,
    pub width: u32,
    pub height: u32,
    pub bytes_per_pixel: u32,
    pub pitch: u32
}

pub struct WindowDimensions {
    pub width: u32,
    pub height: u32
}

#[derive(Default)]
pub struct GameState {
    pub delta_time: f32,
    pub camera: Camera,
    pub last_perf_print: u128
}

#[derive(Default, Clone, Copy)]
pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub y_scale: f32
}

impl Camera {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Camera {
        Camera {
            x,
            y,
            width,
            height,
            y_scale: 1.0
        }
    }

    fn get_bounding_box(self) -> Rectangle {
        Rectangle {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Vector2 {
    x: f32,
    y: f32
}

impl Vector2 {
    fn zero() -> Self {
        Vector2 { x: 0.0, y: 0.0 }
    }
}

impl std::ops::Add for Vector2 {
    type Output = Vector2;

    fn add(self, other: Vector2) -> Vector2 {
        Vector2 {
            x: self.x + other.x,
            y: self.y + other.y
        }
    }
}

impl std::ops::Mul<f32> for Vector2 {
    type Output = Vector2;

    fn mul(self, scalar: f32) -> Vector2 {
        Vector2 {
            x: self.x * scalar,
            y: self.y * scalar
        }
    }
}

#[derive(Clone, Copy)]
struct Vector2u32 {
    x: u32,
    y: u32
}

impl Vector2u32 {
    pub fn new(value: u32) -> Self {
        Vector2u32 {
            x: value,
            y: value
        }
    }
}

impl std::ops::Add<Vector2u32> for Vector2u32 {
    type Output = Vector2u32;

    fn add(self, other: Vector2u32) -> Vector2u32 {
        Vector2u32 {
            x: self.x + other.x,
            y: self.y + other.y
        }
    }
}

impl std::ops::Sub<Vector2u32> for Vector2u32 {
    type Output = Vector2u32;

    fn sub(self, other: Vector2u32) -> Vector2u32 {
        Vector2u32 {
            x: max(self.x as i32 - other.x as i32, 0) as u32,
            y: max(self.y as i32 - other.y as i32, 0) as u32
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Rectangle {
    x: f32,
    y: f32,
    width: f32,
    height: f32
}

impl Rectangle {
    fn intersects(&self, other: Rectangle) -> bool {
        self.x + self.width >= other.x ||
        self.y + self.height >= other.y
    }
}

#[derive(Clone)]
struct BezierCurve {
    p0: Vector2,
    p1: Vector2,
    p2: Vector2,
    p3: Vector2,
    bounding_box: Rectangle
}

impl BezierCurve {
    fn new(p0: Vector2, p1: Vector2, p2: Vector2, p3: Vector2) -> Self {
        let mut curve = BezierCurve {
            p0,
            p1,
            p2,
            p3,
            bounding_box: Rectangle {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0
            }
        };

        curve.bounding_box = curve.get_bounding_box();

        curve
    }

    fn modify(mut self, point: u8, new_position: Vector2) {
        if point == 0 {
            self.p0 = new_position;
        } else if point == 1 {
            self.p1 = new_position;
        } else if point == 2 {
            self.p2 = new_position;
        } else if point == 3 {
            self.p3 = new_position;
        } else {
            eprintln!("{} is not a valid point", point);
        }

        self.bounding_box = self.get_bounding_box();
    }

    // This function was stolen from here:
    // https://youtu.be/aVwxzDHniEw?si=1txEvDjoTSHT0zqk&t=665
    // NOTE:
    // A faster method might be to evaluate the whole curve and look for min and max values
    // Could be worth benchmarking to see what's faster
    // This is obviously more accurate tho
    fn get_bounding_box(&self) -> Rectangle {
        let tx = {
            let a = -3.0 * self.p0.x + 9.0 * self.p1.x -9.0 * self.p2.x + 3.0 * self.p3.x;
            let b = 6.0 * self.p0.x -12.0 * self.p1.x + 6.0 * self.p2.x;
            let c = -3.0 * self.p0.x + 3.0 * self.p1.x;

            let quad = (b.powf(2.0) - 4.0 * a * c).sqrt();

            // Avoid division by zero
            if a.abs() < 1e-6 {
                let tx0 = -c / b;
                (tx0, tx0)
            } else {
                let tx0 = (-b + quad) / (2.0 * a);
                let tx1 = (-b - quad) / (2.0 * a);
                (tx0, tx1)
            }
        };

        let ty = {
            let a = -3.0 * self.p0.y + 9.0 * self.p1.y -9.0 * self.p2.y + 3.0 * self.p3.y;
            let b = 6.0 * self.p0.y -12.0 * self.p1.y + 6.0 * self.p2.y;
            let c = -3.0 * self.p0.y + 3.0 * self.p1.y;

            let quad = (b.powf(2.0) - 4.0 * a * c).sqrt();

            // Avoid division by zero
            if a.abs() < 1e-6 {
                let ty0 = -c / b;
                (ty0, ty0)
            } else {
                let ty0 = (-b + quad) / (2.0 * a);
                let ty1 = (-b - quad) / (2.0 * a);
                (ty0, ty1)
            }
        };

        let mut points: Vec<Vector2> = Vec::new();

        points.push(self.evaluate(0.0));
        points.push(self.evaluate(1.0));

        if tx.0 < 1.0 && tx.0 > 0.0 {
            let point = self.evaluate(tx.0);
            points.push(point);
        }

        if ty.0 < 1.0 && ty.0 > 0.0 {
            let point = self.evaluate(ty.0);
            points.push(point);
        }

        if tx.1 < 1.0 && tx.1 > 0.0 {
            let point = self.evaluate(tx.1);
            points.push(point);
        }

        if ty.1 < 1.0 && ty.1 > 0.0 {
            let point = self.evaluate(ty.1);
            points.push(point);
        }

        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for point in points {
            if point.x < min_x {
                min_x = point.x;
            } else if point.x > max_x {
                max_x = point.x;
            }

            if point.y < min_y {
                min_y = point.y;
            } else if point.y > max_y {
                max_y = point.y;
            }
        }

        Rectangle {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y
        }
    }

    fn evaluate(&self, t: f32) -> Vector2 {
        self.p0 * (-t * -t * -t + 3.0 * t * t - 3.0 * t + 1.0) +
        self.p1 * (3.0 * t * t * t - 6.0 * t * t + 3.0 * t) +
        self.p2 * (-3.0 * t * t * t + 3.0 * t * t) +
        self.p3 * (t * t * t)
    }

    fn derivative(&self, t: f32) -> Vector2 {
        self.p0 * (-3.0 * t * t + 6.0 * t - 3.0) +
        self.p1 * (9.0 * t * t - 12.0 * t + 3.0) +
        self.p2 * (-9.0 * t * t + 6.0 * t) +
        self.p3 * (3.0 * t * t)
    }

    fn second_derivative(&self, t: f32) -> Vector2 {
        self.p0 * (-6.0 * t + 6.0) +
        self.p1 * (18.0 * t - 12.0) +
        self.p2 * (-18.0 * t + 6.0) +
        self.p3 * (6.0 * t)
    }

    // Returns the distance between a point in world space and a specified point on the bezier curve
    fn distance(&self, t: f32, point: Vector2) -> f32 {
        let bezier_point = self.evaluate(t);
        let dx = (bezier_point.x - point.x).abs();
        let dy = (bezier_point.y - point.y).abs();

        (dx * dx + dy * dy).sqrt()
    }

    fn distance_derivative(&self, t: f32, point: Vector2) -> f32 {
        let bezier_point = self.evaluate(t);
        let dx = (bezier_point.x - point.x).abs();
        let dy = (bezier_point.y - point.y).abs();

        let derivative = self.derivative(t);

        (dx * derivative.x + dy * derivative.y) / self.distance(t, point)
    }

    fn distance_second_derivative(&self, t: f32, point: Vector2) -> f32 {
        let dist = self.distance(t, point);
        let dist_deriv = self.distance_derivative(t, point);

        let deriv = self.derivative(t);
        let second_deriv = self.second_derivative(t);

        (dist * second_deriv.x - dist_deriv * dist_deriv * deriv.x) / (dist * dist * dist)
    }

    // Returns the minimum distance between a point and the bezier curve in world space
    fn min_distance(&self, point: Vector2) -> f32 {
        let mut min_dist = f32::MAX;

        let mut t = 0.0;
        while t <= 1.0 {
            let dist = self.distance(t, point);

            if dist < min_dist {
                min_dist = dist;
            }

            t += 0.01;
        }

        min_dist
    }

    fn _min_distance(&self, point: Vector2) -> f32 {
        let mut t = 0.5;

        let mut converged = false;
        while !converged {
            let gradient = self.distance_derivative(t, point);
            if gradient.is_nan() { continue; }
            if gradient.abs() > 1e3 { break; }

            let hessian = self.distance_second_derivative(t, point);

            t = t - gradient / hessian;

            if gradient.abs() < 3.0 {
                converged = true;
            }
        }

        self.distance(t, point)
    }
}

#[no_mangle]
pub unsafe fn game_update_and_render(game_state: &mut GameState, buffer: &mut OffscreenBuffer) {
    clear_buffer(buffer, 0x00000000);

    let bezier = BezierCurve::new(
        Vector2 { x: 0.0, y: 0.5 },
        Vector2 { x: 1.0, y: 0.0 },
        Vector2 { x: 1.0, y: 1.6 },
        Vector2 { x: 0.0, y: 2.0 }
    );

    let mut beziers: Vec<BezierCurve> = Vec::new();
    beziers.push(bezier.clone());

    draw_unit_grid(buffer, game_state.camera);
    draw_circle(buffer, game_state.camera, Vector2::zero(), 0.05, 0xFFFF0000);
    draw_bounding_boxes(buffer, game_state.camera, beziers);
    draw_bezier_curve(buffer, game_state.camera, bezier.clone(), 0.02);

    let start = SystemTime::now();
    let time_now = start.duration_since(UNIX_EPOCH).expect("Time went backwards");
    if game_state.last_perf_print == 0 || time_now.as_millis() - game_state.last_perf_print >= 1000 {
        println!("Frame time: {}", game_state.delta_time);
        println!("FPS: {}", 1000.0 / game_state.delta_time);
        game_state.last_perf_print = time_now.as_millis();
    }
}

// TODO: Make the camera centered on the screen
// Make sure to do this for both functions
fn world_space_to_screen_space(camera: Camera, pos: Vector2) -> Vector2u32 {
    let x = ((pos.x - camera.x) * camera.y_scale) as u32;
    let y = ((pos.y - camera.y) * camera.y_scale) as u32;
    Vector2u32 { x, y }
}

fn screen_space_to_world_space(camera: Camera, pos: Vector2u32) -> Vector2 {
    let x = pos.x as f32 / camera.y_scale + camera.x;
    let y = pos.y as f32 / camera.y_scale + camera.y;
    Vector2 { x, y }
}

unsafe fn clear_buffer(buffer: &mut OffscreenBuffer, color: u32) {
    let mut row: *mut u8 = (*buffer).memory as *mut u8;

    let mut y: u32 = 0;
    while y < (*buffer).height {
        y += 1;

        let mut pixel: *mut u32 = row as *mut u32;

        let mut x: u32 = 0;
        while x < (*buffer).width {
            *pixel = color;

            pixel = pixel.offset(1);
            x += 1;
        }

        row = row.offset((*buffer).pitch as isize);
    }
}

// TODO: Check so positive decimal coordinates for the camera work properly
unsafe fn draw_unit_grid(buffer: &mut OffscreenBuffer, camera: Camera) {
    // Horizontal lines
    let mut screen_x: u32 = 0;
    while screen_x < buffer.width as u32 {
        let mut y: u32 = 0;
        while y <= camera.height as u32 {
            let camera_y_dec = camera.y - (camera.y as i32) as f32;
            let camera_offset_y = (camera_y_dec + y as f32) * camera.y_scale;

            if camera_offset_y > 0.0 {
                // Make sure it doesn't try to draw the last line one pixel off the screen
                if y == camera.height as u32 && camera_y_dec == 0.0 { break; }

                draw_pixel_to_buffer(buffer, screen_x, camera_offset_y as u32, 0xFF444444);
            }

            y += 1;
        }
        screen_x += 1;
    }

    // Vertical lines
    let mut screen_y: u32 = 0;
    while screen_y < buffer.height as u32 {
        let mut x: u32 = 0;
        while x <= camera.width as u32 {
            let camera_x_dec = camera.x - (camera.x as i32) as f32;
            let camera_offset_x = (camera_x_dec + x as f32) * camera.y_scale;

            if camera_offset_x > 0.0 {
                // Make sure it doesn't try to draw the last line one pixel off the screen
                if x == camera.width as u32 && camera_x_dec == 0.0 { break; }

                draw_pixel_to_buffer(buffer, camera_offset_x as u32, screen_y, 0xFF444444);
            }

            x += 1;
        }
        screen_y += 1;
    }
}

unsafe fn draw_bounding_boxes(buffer: &mut OffscreenBuffer, camera: Camera, beziers: Vec<BezierCurve>) {
    let camera_bounding_box = camera.get_bounding_box();

    for bezier in beziers {
        let bounding_box = bezier.bounding_box;

        if !bounding_box.intersects(camera_bounding_box) {
            continue;
        }

        draw_rectangle(buffer, camera, bounding_box, 0x3300DDAA);
    }
}

unsafe fn draw_rectangle(buffer: &mut OffscreenBuffer, camera: Camera, rectangle: Rectangle, color: u32) {
    let rect_top_left = Vector2 {
        x: rectangle.x,
        y: rectangle.y
    };

    let rect_bottom_right = Vector2 {
        x: rectangle.x + rectangle.width,
        y: rectangle.y + rectangle.height
    };

    // Where rectangle starts and ends in screen space
    let rect_top_left_screen = world_space_to_screen_space(camera, rect_top_left);
    let rect_bottom_right_screen = world_space_to_screen_space(camera, rect_bottom_right);

    let start_x = max(rect_top_left_screen.x, 0);
    let start_y = max(rect_top_left_screen.y, 0);
    let end_x = min(rect_bottom_right_screen.x, buffer.width);
    let end_y = min(rect_bottom_right_screen.y, buffer.height);

    let mut x = start_x;
    while x < end_x {
        let mut y = start_y;
        while y < end_y {
            draw_pixel_to_buffer(buffer, x, y, color);
            y += 1;
        }
        x += 1;
    }
}

// TODO: Make it so circles can be drawn partially off-screen
unsafe fn draw_circle(buffer: &mut OffscreenBuffer, camera: Camera, position: Vector2, radius: f32, color: u32) {
    let screen_pos = world_space_to_screen_space(camera, position);
    let screen_radius = (radius * camera.y_scale) as u32;

    if screen_pos.x as i32 - (screen_radius as i32) < 0 ||
       screen_pos.x + screen_radius > buffer.width ||
       screen_pos.y as i32 - (screen_radius as i32) < 0 ||
       screen_pos.y + screen_radius > buffer.height {
        return;
    }

    let mut x = screen_pos.x - screen_radius;
    while x <= screen_pos.x + screen_radius {
        let mut y = screen_pos.y - screen_radius;
        while y <= screen_pos.y + screen_radius {
            let dist = distance(screen_pos, Vector2u32 { x, y });
            if dist <= screen_radius as f32 {
                draw_pixel_to_buffer(buffer, x, y, color);
            } else if dist <= screen_radius as f32 + 1.0 {
                // TODO: Make it so anti-aliasing works properly with transparent circles
                let dist_dec = dist - (dist as i32) as f32;
                let alpha = ((1.0 - dist_dec) * 255.0) as u32;
                let color_with_alpha = (color & 0x00FFFFFF) | (alpha << 24);
                draw_pixel_to_buffer(buffer, x, y, color_with_alpha);
            }
            y += 1;
        }
        x += 1;
    }
}

fn distance(a: Vector2u32, b: Vector2u32) -> f32 {
    let dx = (a.x as f32 - b.x as f32).abs();
    let dy = (a.y as f32 - b.y as f32).abs();
    (dx * dx + dy * dy).sqrt()
}

unsafe fn draw_bezier_curve(buffer: &mut OffscreenBuffer, camera: Camera, bezier: BezierCurve, radius: f32) {
    let bounding_box = bezier.bounding_box;

    let top_left = Vector2 { x: bounding_box.x, y: bounding_box.y };
    let bottom_right = Vector2 { x: bounding_box.x + bounding_box.width, y: bounding_box.y + bounding_box.height };

    let top_left_screen_space = world_space_to_screen_space(camera, top_left) - Vector2u32::new(radius as u32 + 2);
    let bottom_right_screen_space = world_space_to_screen_space(camera, bottom_right) + Vector2u32::new(radius as u32 + 2);

    let mut x = top_left_screen_space.x;
    while x <= bottom_right_screen_space.x {
        let mut y = top_left_screen_space.y;
        while y <= bottom_right_screen_space.y {
            let world_pos = screen_space_to_world_space(camera, Vector2u32 { x, y });
            let world_distance = bezier.min_distance(world_pos);
            let screen_distance = world_distance * camera.y_scale;
            let screen_radius = radius * camera.y_scale;

            if screen_distance <= screen_radius {
                draw_pixel_to_buffer(buffer, x, y, 0xFFFFFFFF);
            }

            y += 1;
        }
        x += 1;
    }
}

unsafe fn draw_pixel_to_buffer(buffer: &mut OffscreenBuffer, x: u32, y: u32, color: u32) {
    let mut row: *mut u8 = (*buffer).memory as *mut u8;
    row = row.offset((*buffer).pitch as isize * y as isize);

    let mut pixel: *mut u32 = row as *mut u32;
    pixel = pixel.offset(x as isize);

    let alpha = get_alpha(color);
    if alpha == 1.0 {
        *pixel = color;
    } else {
        *pixel = lerp_color(*pixel, color, alpha);
    }
}

fn get_alpha(color: u32) -> f32 {
    (color >> 24) as f32 / 255.0
}

fn lerp_color(a: u32, b: u32, t: f32) -> u32 {
    let a_red = (a >> 16) as u8;
    let a_green = (a >> 8) as u8;
    let a_blue = a as u8;

    let b_red = (b >> 16) as u8;
    let b_green = (b >> 8) as u8;
    let b_blue = b as u8;

    let red = (a_red as f32 + t * (b_red as f32 - a_red as f32)) as u8;
    let green = (a_green as f32 + t * (b_green as f32 - a_green as f32)) as u8;
    let blue = (a_blue as f32 + t * (b_blue as f32 - a_blue as f32)) as u8;

    // NOTE: This version uses fewer variables, if a performance concern
    //let red = (a >> 16) as u8 + (t * ((b >> 16) as u8 - (a >> 16) as u8) as f32) as u8;
    //let green = (a >> 8) as u8 + (t * ((b >> 8) as u8 - (a >> 8) as u8) as f32) as u8;
    //let blue = a as u8 + (t * (b as u8 - a as u8) as f32) as u8;

    ((red as u32) << 16) | ((green as u32) << 8) | blue as u32
}
