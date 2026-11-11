use schemars::JsonSchema;
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ElementSelector {
    pub identifier: Option<String>,
    pub role: Option<String>,
    pub title: Option<String>,
}

impl ElementSelector {
    pub fn is_empty(&self) -> bool {
        self.identifier.is_none() && self.role.is_none() && self.title.is_none()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Eq for Point {}

#[derive(Clone, Debug, PartialEq)]
pub enum Location {
    Element(ElementSelector),
    Point(Point),
}

impl Eq for Location {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ClickMethod {
    Auto,
    Ax,
    Mouse,
}

impl ClickMethod {
    pub const ALL: [&'static str; 3] = ["auto", "ax", "mouse"];

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "auto" => Some(Self::Auto),
            "ax" => Some(Self::Ax),
            "mouse" => Some(Self::Mouse),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    pub const ALL: [&'static str; 3] = ["left", "right", "middle"];

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "middle" => Some(Self::Middle),
            _ => None,
        }
    }

    pub fn number(self) -> i64 {
        match self {
            Self::Left => 0,
            Self::Right => 1,
            Self::Middle => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ScrollUnit {
    Pixel,
    Line,
}

impl ScrollUnit {
    pub const ALL: [&'static str; 2] = ["pixel", "line"];

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "pixel" => Some(Self::Pixel),
            "line" => Some(Self::Line),
            _ => None,
        }
    }

    pub fn maximum_magnitude(self) -> i32 {
        match self {
            Self::Pixel => 10_000,
            Self::Line => 1_000,
        }
    }
}
