#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const COMMAND: Self = Self(1);
    pub const CONTROL: Self = Self(2);
    pub const OPTION: Self = Self(4);
    pub const SHIFT: Self = Self(8);
    pub const ORDER: [Self; 4] = [Self::COMMAND, Self::CONTROL, Self::OPTION, Self::SHIFT];

    pub fn parse(text: &str) -> Result<Self, String> {
        Self::parse_tokens(text, true)
    }

    pub fn parse_click(text: &str) -> Result<Self, String> {
        Self::parse_tokens(text, false)
    }

    fn parse_tokens(text: &str, aliases: bool) -> Result<Self, String> {
        let mut modifiers = Self::default();
        if text.is_empty() {
            return Err("expected one or more comma-separated modifiers".to_owned());
        }
        for token in text.split(',') {
            let allowed = match token {
                "cmd" | "ctrl" | "alt" | "shift" => true,
                "command" | "control" | "option" | "opt" => aliases,
                _ => false,
            };
            if !allowed {
                return Err(format!("unknown modifier {token}"));
            }
            let modifier =
                Self::parse_token(token).ok_or_else(|| format!("unknown modifier {token}"))?;
            if modifiers.contains(modifier) {
                return Err(format!("duplicate modifier {token}"));
            }
            modifiers.0 |= modifier.0;
        }
        Ok(modifiers)
    }

    fn parse_token(token: &str) -> Option<Self> {
        match token.to_ascii_lowercase().as_str() {
            "cmd" | "command" => Some(Self::COMMAND),
            "ctrl" | "control" => Some(Self::CONTROL),
            "alt" | "option" | "opt" => Some(Self::OPTION),
            "shift" => Some(Self::SHIFT),
            _ => None,
        }
    }

    pub fn contains(self, modifier: Self) -> bool {
        self.0 & modifier.0 == modifier.0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub(crate) fn event_flags(self) -> u64 {
        let mut flags = 0;
        if self.contains(Self::COMMAND) {
            flags |= 0x100000;
        }
        if self.contains(Self::CONTROL) {
            flags |= 0x40000;
        }
        if self.contains(Self::OPTION) {
            flags |= 0x80000;
        }
        if self.contains(Self::SHIFT) {
            flags |= 0x20000;
        }
        flags
    }

    pub(crate) fn key_code(self) -> u16 {
        match self {
            Self::COMMAND => 55,
            Self::CONTROL => 59,
            Self::OPTION => 58,
            Self::SHIFT => 56,
            _ => unreachable!("modifier order"),
        }
    }

    pub fn names(self) -> Vec<&'static str> {
        Self::ORDER
            .into_iter()
            .filter(|modifier| self.contains(*modifier))
            .map(|modifier| match modifier {
                Self::COMMAND => "cmd",
                Self::CONTROL => "ctrl",
                Self::OPTION => "alt",
                Self::SHIFT => "shift",
                _ => unreachable!("modifier order"),
            })
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Chord {
    pub modifiers: Modifiers,
    pub key: BaseKey,
}

impl Chord {
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.is_empty() || text.chars().any(char::is_whitespace) {
            return Err(
                "expected + separated modifiers and exactly one base key, without whitespace"
                    .to_owned(),
            );
        }
        let mut modifiers = Modifiers::default();
        let mut key = None;
        for token in text.split('+') {
            if token.is_empty() {
                return Err("empty chord token".to_owned());
            }
            if let Some(modifier) = Modifiers::parse_token(token) {
                if modifiers.contains(modifier) {
                    return Err(format!("duplicate modifier {token}"));
                }
                modifiers.0 |= modifier.0;
                continue;
            }
            if key.is_some() {
                return Err("a chord must contain exactly one base key".to_owned());
            }
            key =
                Some(BaseKey::parse(token).ok_or_else(|| format!("unsupported base key {token}"))?);
        }
        Ok(Self { modifiers, key: key.ok_or_else(|| "a chord requires one base key".to_owned())? })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BaseKey {
    Character(char),
    Named(&'static str),
}

impl BaseKey {
    fn parse(token: &str) -> Option<Self> {
        let lowercase = token.to_ascii_lowercase();
        let mut characters = lowercase.chars();
        if let (Some(character), None) = (characters.next(), characters.next())
            && (character.is_ascii_alphanumeric() || "`-=[]\\;',./".contains(character))
        {
            return Some(Self::Character(character));
        }
        let named = match lowercase.as_str() {
            "return" => "return",
            "tab" => "tab",
            "space" => "space",
            "escape" | "esc" => "escape",
            "delete" => "delete",
            "forward-delete" => "forward-delete",
            "left" => "left",
            "right" => "right",
            "up" => "up",
            "down" => "down",
            "home" => "home",
            "end" => "end",
            "pageup" => "pageup",
            "pagedown" => "pagedown",
            "f1" => "f1",
            "f2" => "f2",
            "f3" => "f3",
            "f4" => "f4",
            "f5" => "f5",
            "f6" => "f6",
            "f7" => "f7",
            "f8" => "f8",
            "f9" => "f9",
            "f10" => "f10",
            "f11" => "f11",
            "f12" => "f12",
            _ => return None,
        };
        Some(Self::Named(named))
    }

    pub fn name(&self) -> String {
        match self {
            Self::Character(character) => character.to_string(),
            Self::Named(name) => (*name).to_owned(),
        }
    }
}
