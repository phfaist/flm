//! The length value of the presentation vocabulary: an exact amount and a
//! unit, which is how a payload states a dimension.
//!
//! A **length** ([`Length`]) is what a payload carries wherever it states a
//! distance or a width: the indentation of a line of a `flm.pres.lines` node
//! ([`LinePayload`](super::LinePayload)), the size an image is shown at
//! ([`ImagePayload`](super::ImagePayload)). It is never a bare number: a bare
//! number would leave each backend to guess what it counts, and a count of
//! columns or of indentation steps is too coarse to express what an author
//! means. A length is written, parsed and projected as its decimal amount
//! followed directly by its unit, with no space: `1tab`, `2tab`, `1.5em`,
//! `10pt`, `0.25in`, `3mm`.
//!
//! # The amount is exact
//!
//! The amount is a fixed-point number with sixteen fractional bits
//! ([`I48F16`], from the `fixed` crate), the model TeX uses for its own
//! dimensions: one unit of the fraction is one *scaled point*, 1/65536 of the
//! unit. It is exact rather than a floating-point number so that two payloads
//! holding the same length compare equal and hash alike, and so that the
//! output written from a length is byte-identical on every platform and every
//! run. A decimal amount that is not a whole number of scaled points, such as
//! `0.1`, is rounded to the nearest scaled point when it is parsed, and
//! projected as the shortest decimal that parses back to the same amount.
//!
//! # The units
//!
//! The units form a closed set ([`LengthUnit`]):
//!
//! - `tab` — a **tab**, the unit each backend sets the width of through one
//!   setting of its own (the text backend's render option `tab_width`, for
//!   instance). A line indented by `\indent` is indented by one tab.
//! - `em` — the width of an em in the current font.
//! - `pt` — a **point**, 1/72 inch: the point of CSS and of PostScript. It is
//!   the only point this vocabulary has; LaTeX writes this unit `bp` (a "big
//!   point"), and TeX's own point of 1/72.27 inch has no unit here.
//! - `mm`, `cm`, `in` — millimeters, centimeters and inches.
//!
//! The set is closed so that every backend can write every length: a unit a
//! backend did not support would need a second way of writing the length. A
//! unit added later changes no projection already written, since each length
//! writes its own unit.
//!
//! # Conversion among the physical units
//!
//! Four of the units have a fixed size — `pt`, `mm`, `cm` and `in`, the
//! **physical units** ([`LengthUnit::is_physical`]) — and a length in one of
//! them converts to any other of them: [`Length::convert_to`] writes a length
//! in another unit, [`Length::scaled`] multiplies a length by the ratio of two
//! others (how an image given one dimension gets the other while keeping its
//! proportions), [`Length::from_dots`] gives the size of a number of dots
//! printed at a resolution, and [`Length::times`] multiplies a length by a
//! factor (how an image is magnified for one output; any unit, kept). Each computes its result exactly, as a fraction,
//! and rounds it **once**, at the end, to the nearest scaled point of the
//! result's unit, a half rounded away from zero; the four units' sizes are
//! whole multiples of 1/9144 inch (a point is 127 of them, a millimeter 360),
//! so no size is itself rounded. A tab and an em have no fixed size and
//! convert to nothing: a conversion involving one is refused with a
//! [`LengthConversionError`], as is a result too large to hold.

use std::fmt;

pub use fixed::types::I48F16;

/// The unit of a [`Length`]: one of a closed set, spelled `tab`, `em`, `pt`,
/// `mm`, `cm` and `in` ([`LengthUnit::as_str`]).
///
/// See the [module documentation](self) for what each unit measures, the
/// point in particular; in short, a tab is the unit each backend sets the
/// width of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LengthUnit {
    /// A tab, `tab`: the width each backend sets through one setting of its
    /// own.
    Tabs,
    /// An em, `em`: the width of an em in the current font.
    Ems,
    /// A point, `pt` (see the [module documentation](self)).
    Points,
    /// A millimeter, `mm`.
    Millimeters,
    /// A centimeter, `cm`.
    Centimeters,
    /// An inch, `in`.
    Inches,
}

impl LengthUnit {
    /// Every unit, in declaration order.
    pub const ALL: [LengthUnit; 6] = [
        LengthUnit::Tabs,
        LengthUnit::Ems,
        LengthUnit::Points,
        LengthUnit::Millimeters,
        LengthUnit::Centimeters,
        LengthUnit::Inches,
    ];

    /// The unit's stable name, as a length is written and projected: `tab`,
    /// `em`, `pt`, `mm`, `cm` or `in`.
    pub fn as_str(self) -> &'static str {
        match self {
            LengthUnit::Tabs => "tab",
            LengthUnit::Ems => "em",
            LengthUnit::Points => "pt",
            LengthUnit::Millimeters => "mm",
            LengthUnit::Centimeters => "cm",
            LengthUnit::Inches => "in",
        }
    }

    /// The unit of a stable name, if any.
    pub fn parse(name: &str) -> Option<LengthUnit> {
        LengthUnit::ALL.into_iter().find(|unit| unit.as_str() == name)
    }

    /// Whether the unit has a fixed size — `pt`, `mm`, `cm` or `in`, the
    /// units a length converts among (see the
    /// [module documentation](self#conversion-among-the-physical-units)).
    /// A tab and an em are not physical.
    pub fn is_physical(self) -> bool {
        self.size().is_some()
    }

    /// The unit's size in 1/9144 inch, the largest length of which all four
    /// physical units are whole multiples (72 × 127 = 9144 = 25.4 × 360);
    /// `None` for a unit with no fixed size.
    fn size(self) -> Option<i128> {
        match self {
            LengthUnit::Tabs | LengthUnit::Ems => None,
            LengthUnit::Points => Some(127),
            LengthUnit::Millimeters => Some(360),
            LengthUnit::Centimeters => Some(3600),
            LengthUnit::Inches => Some(9144),
        }
    }
}

impl fmt::Display for LengthUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A dimension: an exact amount of a [`LengthUnit`].
///
/// A length is written and projected as its decimal amount followed directly
/// by its unit — `1tab`, `1.5em`, `10pt` — through its `Display`, and read
/// back from that text by [`Length::parse`]. Two equal lengths compare equal
/// and hash alike; a length is never ordered against another, since `1in` and
/// `2pt` have no order a derived comparison would get right. The default is
/// the zero length, `0tab`.
///
/// See the [module documentation](self) for why the amount is exact and what
/// each unit measures.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::pres::{Length, LengthUnit};
///
/// let indent = Length::parse("1.5em")?;
/// assert_eq!(indent.unit, LengthUnit::Ems);
/// assert_eq!(indent.to_string(), "1.5em");
/// assert_eq!(Length::tabs(2).to_string(), "2tab");
/// assert!(Length::parse("1.5").is_err());
/// # Ok::<(), flm_core::stdcore::backends::pres::LengthError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Length {
    /// The amount, exact to one scaled point (1/65536 of the unit).
    pub amount: I48F16,
    /// The unit the amount counts.
    pub unit: LengthUnit,
}

impl Default for Length {
    fn default() -> Length {
        Length::tabs(0)
    }
}

impl Length {
    /// The length of `amount` units of `unit`.
    pub fn new(amount: I48F16, unit: LengthUnit) -> Length {
        Length { amount, unit }
    }

    /// A whole number of tabs: `count` units of [`LengthUnit::Tabs`], how a
    /// line indented by `count` `\indent` commands is indented.
    pub fn tabs(count: i32) -> Length {
        // A 32-bit count shifted by the sixteen fractional bits fits the
        // amount's 64 bits, so the amount is exact.
        Length::new(I48F16::from_bits(i64::from(count) << I48F16::FRAC_NBITS), LengthUnit::Tabs)
    }

    /// Whether the amount is zero, regardless of the unit.
    pub fn is_zero(&self) -> bool {
        self.amount == I48F16::ZERO
    }

    /// The length written as `text`: a decimal amount followed directly by a
    /// unit's stable name, such as `1tab`, `1.5em`, `-2pt` or `.5in`.
    ///
    /// The amount is an optional sign, digits, and optionally a full stop
    /// followed by digits, with at least one digit in all; it is rounded to
    /// the nearest scaled point (1/65536 of the unit). No space may be written
    /// between the amount and the unit, nor around the length.
    ///
    /// # Errors
    ///
    /// A [`LengthError`] saying what is wrong: an empty text, a text with no
    /// unit (`1.5`), an unknown unit (`2xx`), no amount before the unit
    /// (`pt`), an amount that is not a decimal number (`1.2.3pt`, `1e3pt`),
    /// or an amount too large to hold.
    pub fn parse(text: &str) -> Result<Length, LengthError> {
        if text.is_empty() {
            return Err(LengthError::Empty);
        }
        let amount_end = text.trim_end_matches(|ch: char| ch.is_ascii_alphabetic()).len();
        let (amount, unit) = text.split_at(amount_end);
        if unit.is_empty() {
            return Err(LengthError::NoUnit { text: text.to_string() });
        }
        let unit = LengthUnit::parse(unit)
            .ok_or_else(|| LengthError::UnknownUnit { unit: unit.to_string() })?;
        if amount.is_empty() {
            return Err(LengthError::NoAmount { text: text.to_string() });
        }
        if !is_decimal(amount) {
            return Err(LengthError::BadAmount { text: text.to_string(), amount: amount.to_string() });
        }
        let value: I48F16 = amount
            .parse()
            .map_err(|_| LengthError::Overflow { text: text.to_string(), amount: amount.to_string() })?;
        Ok(Length::new(value, unit))
    }
}

/// Whether `text` is an optional sign followed by digits with at most one
/// full stop among them, holding at least one digit: the only form of amount
/// [`Length::parse`] accepts. The `fixed` crate's own reader admits more (an
/// exponent, underscores), which a written length does not.
fn is_decimal(text: &str) -> bool {
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    let all_digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
    !(whole.is_empty() && fraction.is_empty()) && all_digits(whole) && all_digits(fraction)
}

impl Length {
    /// The same length written in `unit`: `2.54cm` in inches is `1in`.
    ///
    /// The conversion is exact but for one rounding of the result to the
    /// nearest scaled point of `unit`, a half rounded away from zero (see the
    /// [module documentation](self#conversion-among-the-physical-units)).
    ///
    /// # Errors
    ///
    /// [`LengthConversionError::NotPhysical`] where the length's unit or
    /// `unit` is a tab or an em, and [`LengthConversionError::Overflow`]
    /// where the result is too large to hold.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdcore::backends::pres::{Length, LengthUnit};
    ///
    /// let inch = Length::parse("2.54cm")?.convert_to(LengthUnit::Inches)?;
    /// assert_eq!(inch.to_string(), "1in");
    /// assert!(Length::parse("1em")?.convert_to(LengthUnit::Points).is_err());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn convert_to(&self, unit: LengthUnit) -> Result<Length, LengthConversionError> {
        let from = physical_size(self.unit)?;
        let to = physical_size(unit)?;
        let bits = divide_rounded(i128::from(self.amount.to_bits()) * from, to)?;
        Ok(Length::new(I48F16::from_bits(bits), unit))
    }

    /// This length multiplied by the ratio `numerator ÷ denominator` of two
    /// lengths, written in `unit`.
    ///
    /// This is how one dimension of a picture follows from the other while
    /// keeping its proportions: a picture 340pt wide and 170pt high shown
    /// 5cm wide is `170pt` scaled by `5cm ÷ 340pt`, written in `cm`, a height
    /// of `2.5cm`. The three lengths may be in any physical units; the result
    /// is computed exactly and rounded once, to the nearest scaled point of
    /// `unit`, a half rounded away from zero.
    ///
    /// # Errors
    ///
    /// [`LengthConversionError::NotPhysical`] where any of the three lengths
    /// or `unit` is a tab or an em; [`LengthConversionError::ZeroDenominator`]
    /// where `denominator` is zero; [`LengthConversionError::Overflow`] where
    /// the result is too large to hold.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdcore::backends::pres::{Length, LengthUnit};
    ///
    /// let height = Length::parse("170pt")?;
    /// let given_width = Length::parse("5cm")?;
    /// let width = Length::parse("340pt")?;
    /// let shown = height.scaled(&given_width, &width, given_width.unit)?;
    /// assert_eq!(shown.to_string(), "2.5cm");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn scaled(
        &self,
        numerator: &Length,
        denominator: &Length,
        unit: LengthUnit,
    ) -> Result<Length, LengthConversionError> {
        let value = i128::from(self.amount.to_bits()) * physical_size(self.unit)?;
        let numerator =
            i128::from(numerator.amount.to_bits()) * physical_size(numerator.unit)?;
        let denominator =
            i128::from(denominator.amount.to_bits()) * physical_size(denominator.unit)?;
        let to = physical_size(unit)?;
        if denominator == 0 {
            return Err(LengthConversionError::ZeroDenominator);
        }
        // value × numerator overflows 128 bits only for amounts near 2^47
        // units; checked, and reported as a result too large to hold.
        let product = value.checked_mul(numerator).ok_or(LengthConversionError::Overflow)?;
        let divisor = denominator.checked_mul(to).ok_or(LengthConversionError::Overflow)?;
        let bits = divide_rounded(product, divisor)?;
        Ok(Length::new(I48F16::from_bits(bits), unit))
    }

    /// This length multiplied by `factor`, in the same unit.
    ///
    /// This is how an image is magnified for one output. The product is
    /// computed exactly and rounded once, to the nearest scaled point of the
    /// unit, a half rounded away from zero. Any unit is accepted, since the
    /// unit does not change.
    ///
    /// # Errors
    ///
    /// [`LengthConversionError::Overflow`] where the result is too large to
    /// hold.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdcore::backends::pres::{I48F16, Length};
    ///
    /// let width = Length::parse("4cm")?;
    /// assert_eq!(width.times(I48F16::from_num(1.25))?.to_string(), "5cm");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn times(&self, factor: I48F16) -> Result<Length, LengthConversionError> {
        let product = i128::from(self.amount.to_bits()) * i128::from(factor.to_bits());
        let bits = divide_rounded(product, 1_i128 << I48F16::FRAC_NBITS)?;
        Ok(Length::new(I48F16::from_bits(bits), self.unit))
    }

    /// The size, in points, of `dots` dots printed at `dots_per_inch` dots
    /// per inch: `dots × 72 ÷ dots_per_inch` points, a point being 1/72 inch.
    ///
    /// This is the physical size of a picture of `dots` pixels at a stated
    /// resolution. It is computed exactly and rounded once, to the nearest
    /// scaled point (1/65536 pt), a half rounded away from zero.
    ///
    /// # Errors
    ///
    /// [`LengthConversionError::ZeroDenominator`] where `dots_per_inch` is
    /// zero, [`LengthConversionError::Overflow`] where the result is too
    /// large to hold. A negative resolution gives a negative length.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdcore::backends::pres::{I48F16, Length};
    ///
    /// assert_eq!(Length::from_dots(300, I48F16::from_num(150))?.to_string(), "144pt");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_dots(dots: u64, dots_per_inch: I48F16) -> Result<Length, LengthConversionError> {
        let resolution = i128::from(dots_per_inch.to_bits());
        if resolution == 0 {
            return Err(LengthConversionError::ZeroDenominator);
        }
        // dots × 72 points, as scaled points, over the resolution's own
        // scaled count: the two scalings of the resolution cancel once the
        // numerator carries one more.
        let unit = 1_i128 << I48F16::FRAC_NBITS;
        let numerator = i128::from(dots) * 72 * unit * unit;
        let bits = divide_rounded(numerator, resolution)?;
        Ok(Length::new(I48F16::from_bits(bits), LengthUnit::Points))
    }

    /// The amount alone, in the decimal a length is projected with — the
    /// shortest that reads back to the same amount (`2`, `1.5`, `-0.25`) —
    /// for a backend that writes the unit its own way: the page writes a tab
    /// as a multiple of a custom property, LaTeX a point as `bp`.
    pub(crate) fn amount_decimal(&self) -> String {
        decimal(self.amount)
    }
}

impl fmt::Display for Length {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.amount_decimal(), self.unit)
    }
}

/// The shortest decimal that [`Length::parse`] reads back to `amount`: the
/// whole part, then as few fractional digits as that takes, none for a whole
/// number, a minus sign before a negative amount.
///
/// The `fixed` crate's own printing is not used: it prints the shortest
/// decimal nearer to the amount than to its neighbors, which does not always
/// read back to it (`0.1` for 6553 scaled points, which reads as 6554).
// See decision rationale at [§flm-dd-dr:rend-pres-length].
fn decimal(amount: I48F16) -> String {
    let bits = i128::from(amount.to_bits());
    let magnitude = bits.unsigned_abs();
    let unit = 1_u128 << I48F16::FRAC_NBITS;
    let (whole, fraction) = (magnitude / unit, magnitude % unit);
    let sign = if bits < 0 { "-" } else { "" };
    // Sixteen fractional bits need at most sixteen decimal digits to be
    // written exactly, so the loop always ends by returning.
    for digits in 0..=I48F16::FRAC_NBITS {
        let scale = 10_u128.pow(digits);
        // The fraction rounded to `digits` digits, a half rounded up.
        let rounded = (2 * fraction * scale + unit) / (2 * unit);
        let (whole, rounded) = if rounded == scale { (whole + 1, 0) } else { (whole, rounded) };
        let text = if digits == 0 {
            format!("{sign}{whole}")
        } else {
            format!("{sign}{whole}.{rounded:0width$}", width = digits as usize)
        };
        // The first number of digits that reads back ends in no zero: the
        // same text without it would have read back one digit earlier.
        if text.parse::<I48F16>() == Ok(amount) {
            return text;
        }
    }
    format!("{amount}")
}

/// The size of a physical unit in 1/9144 inch, or the refusal of a unit
/// with no fixed size.
fn physical_size(unit: LengthUnit) -> Result<i128, LengthConversionError> {
    unit.size().ok_or(LengthConversionError::NotPhysical { unit })
}

/// `numerator ÷ denominator` rounded to the nearest whole number, a half
/// rounded away from zero, as the bits of an amount; the only rounding of
/// every conversion. `denominator` is not zero.
fn divide_rounded(numerator: i128, denominator: i128) -> Result<i64, LengthConversionError> {
    let (numerator, denominator) =
        if denominator < 0 { (-numerator, -denominator) } else { (numerator, denominator) };
    let magnitude = numerator.unsigned_abs();
    let denominator = denominator.unsigned_abs();
    let rounded = (magnitude / denominator) + u128::from(2 * (magnitude % denominator) >= denominator);
    let rounded = i128::try_from(rounded).map_err(|_| LengthConversionError::Overflow)?;
    let rounded = if numerator < 0 { -rounded } else { rounded };
    i64::try_from(rounded).map_err(|_| LengthConversionError::Overflow)
}

/// Why a length does not convert: the error [`Length::convert_to`],
/// [`Length::scaled`], [`Length::from_dots`] and [`Length::times`] return
/// when they refuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthConversionError {
    /// A length or the requested unit is a tab or an em, which has no fixed
    /// size and converts to no other unit.
    NotPhysical {
        /// The unit with no fixed size.
        unit: LengthUnit,
    },
    /// The ratio's denominator, or the resolution, is zero.
    ZeroDenominator,
    /// The result is too large to hold.
    Overflow,
}

impl fmt::Display for LengthConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LengthConversionError::NotPhysical { unit } => write!(
                f,
                "a length in `{unit}` has no fixed size and cannot be converted to another unit"
            ),
            LengthConversionError::ZeroDenominator => {
                f.write_str("a length cannot be divided by zero")
            }
            LengthConversionError::Overflow => f.write_str("the converted length is too large"),
        }
    }
}

impl std::error::Error for LengthConversionError {}

/// Why a text is not a [`Length`]: the error [`Length::parse`] returns when
/// it refuses the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LengthError {
    /// The text is empty.
    Empty,
    /// The text ends in no unit, as `1.5` does.
    NoUnit {
        /// The text as written.
        text: String,
    },
    /// The text ends in letters that are no unit, as `2xx` does.
    UnknownUnit {
        /// The letters at the end of the text.
        unit: String,
    },
    /// No amount is written before the unit, as in `pt`.
    NoAmount {
        /// The text as written.
        text: String,
    },
    /// The text before the unit is not a decimal number.
    BadAmount {
        /// The text as written.
        text: String,
        /// The amount as written.
        amount: String,
    },
    /// The amount is too large to hold.
    Overflow {
        /// The text as written.
        text: String,
        /// The amount as written.
        amount: String,
    },
}

impl fmt::Display for LengthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let units = || LengthUnit::ALL.map(LengthUnit::as_str).join(", ");
        match self {
            LengthError::Empty => f.write_str("a length cannot be empty"),
            LengthError::NoUnit { text } => {
                write!(f, "the length `{text}` has no unit; write one of {}", units())
            }
            LengthError::UnknownUnit { unit } => {
                write!(f, "`{unit}` is not a unit of length; write one of {}", units())
            }
            LengthError::NoAmount { text } => {
                write!(f, "the length `{text}` must begin with an amount, a decimal number")
            }
            LengthError::BadAmount { text, amount } => {
                write!(
                    f,
                    "the length `{text}` must begin with a decimal number, such as `2`, `-0.5` \
                     or `1.25`, but its amount `{amount}` is not one"
                )
            }
            LengthError::Overflow { text, amount } => {
                write!(f, "the amount `{amount}` of the length `{text}` is too large")
            }
        }
    }
}

impl std::error::Error for LengthError {}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use super::*;

    fn hash_of(length: &Length) -> u64 {
        let mut hasher = DefaultHasher::new();
        length.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn a_length_parses_and_projects_back_unchanged() {
        for (text, unit) in [
            ("1tab", LengthUnit::Tabs),
            ("2tab", LengthUnit::Tabs),
            ("1.5em", LengthUnit::Ems),
            ("10pt", LengthUnit::Points),
            ("0.25in", LengthUnit::Inches),
            ("3mm", LengthUnit::Millimeters),
            ("2.54cm", LengthUnit::Centimeters),
            ("-2pt", LengthUnit::Points),
            ("0.1em", LengthUnit::Ems),
        ] {
            let length = Length::parse(text).unwrap_or_else(|error| panic!("{text}: {error}"));
            assert_eq!(length.unit, unit, "{text}");
            assert_eq!(length.to_string(), text);
            assert_eq!(Length::parse(&length.to_string()), Ok(length), "{text}");
        }
    }

    #[test]
    fn an_amount_is_rounded_to_a_scaled_point_and_projected_shortest() {
        let length = Length::parse("1.50em").expect("a length");
        assert_eq!(length.amount, I48F16::from_bits(3 << 15));
        assert_eq!(length.to_string(), "1.5em");
        assert_eq!(Length::parse(".5in").expect("a length").to_string(), "0.5in");
        assert_eq!(Length::parse("+2pt").expect("a length").to_string(), "2pt");
        assert_eq!(Length::parse("3.pt").expect("a length").to_string(), "3pt");
        // One scaled point, and every amount of the fraction, read back.
        for bits in [1_i64, 3, 6553, 6554, 32767, 65535, 65537, -6553, i64::MAX, i64::MIN] {
            let length = Length::new(I48F16::from_bits(bits), LengthUnit::Ems);
            assert_eq!(Length::parse(&length.to_string()), Ok(length), "{bits}");
        }
    }

    #[test]
    fn the_tab_constructor_counts_whole_tabs() {
        assert_eq!(Length::tabs(1), Length::parse("1tab").expect("a length"));
        assert_eq!(Length::tabs(2).to_string(), "2tab");
        assert_eq!(Length::tabs(i32::MAX).amount, I48F16::from_num(i32::MAX));
        assert_eq!(Length::tabs(-3).to_string(), "-3tab");
    }

    #[test]
    fn a_zero_length_is_zero_whatever_its_unit() {
        let zero = Length::default();
        assert!(zero.is_zero());
        assert_eq!(zero.to_string(), "0tab");
        assert!(Length::parse("0pt").expect("a length").is_zero());
        assert!(Length::parse("-0em").expect("a length").is_zero());
        assert!(!Length::tabs(1).is_zero());
    }

    #[test]
    fn a_text_that_is_no_length_is_refused_with_its_reason() {
        assert_eq!(Length::parse(""), Err(LengthError::Empty));
        assert!(matches!(Length::parse("1.5"), Err(LengthError::NoUnit { .. })));
        assert_eq!(
            Length::parse("2xx"),
            Err(LengthError::UnknownUnit { unit: String::from("xx") })
        );
        assert!(matches!(Length::parse("pt"), Err(LengthError::NoAmount { .. })));
        assert!(matches!(Length::parse("infpt"), Err(LengthError::UnknownUnit { .. })));
        assert!(matches!(Length::parse("nan"), Err(LengthError::UnknownUnit { .. })));
        for text in ["1.2.3pt", "1e3pt", "1_0pt", "1 pt", " 1pt", "-pt", ".pt", "1,5em"] {
            assert!(Length::parse(text).is_err(), "{text}");
        }
        assert!(matches!(
            Length::parse("1000000000000000000000pt"),
            Err(LengthError::Overflow { .. })
        ));
        assert!(Length::parse("1Pt").is_err(), "units are lowercase");
    }

    #[test]
    fn two_equal_lengths_hash_alike() {
        let a = Length::parse("1.5em").expect("a length");
        let b = Length::new(I48F16::from_bits(3 << 15), LengthUnit::Ems);
        assert_eq!(a, b);
        assert_eq!(hash_of(&a), hash_of(&b));
        assert_ne!(a, Length::parse("1.5pt").expect("a length"));
    }

    #[test]
    fn every_unit_parses_from_its_spelling() {
        for unit in LengthUnit::ALL {
            assert_eq!(LengthUnit::parse(unit.as_str()), Some(unit));
            assert_eq!(unit.to_string(), unit.as_str());
        }
        assert_eq!(LengthUnit::parse("bp"), None);
    }

    fn length(text: &str) -> Length {
        Length::parse(text).unwrap_or_else(|error| panic!("{text}: {error}"))
    }

    #[test]
    fn every_pair_of_physical_units_converts_both_ways() {
        // One inch in each physical unit, exactly.
        let inch = [("72pt", LengthUnit::Points), ("25.4mm", LengthUnit::Millimeters),
            ("2.54cm", LengthUnit::Centimeters), ("1in", LengthUnit::Inches)];
        for (from_text, from_unit) in inch {
            for (to_text, to_unit) in inch {
                let from = length(from_text);
                let converted = from.convert_to(to_unit).expect("a physical conversion");
                // 25.4 and 2.54 are no whole number of scaled points: each
                // parsed amount is off by at most half a scaled point of its
                // unit, which the conversion scales by the ratio of the units,
                // and the conversion rounds once more.
                let expected = length(to_text);
                let ratio = from_unit.size().unwrap() / to_unit.size().unwrap();
                let bound = i64::try_from(ratio).unwrap() / 2 + 2;
                let off = (converted.amount.to_bits() - expected.amount.to_bits()).abs();
                assert!(off <= bound, "{from_text} in {to_unit}: {converted}, not {to_text}");
                assert_eq!(converted.unit, to_unit);
            }
        }
        // Amounts that are exact in both units convert exactly.
        for (from, unit, to) in [
            ("72pt", LengthUnit::Inches, "1in"),
            ("1in", LengthUnit::Points, "72pt"),
            ("10mm", LengthUnit::Centimeters, "1cm"),
            ("1cm", LengthUnit::Millimeters, "10mm"),
            ("0.5in", LengthUnit::Points, "36pt"),
            ("36pt", LengthUnit::Inches, "0.5in"),
            ("-1in", LengthUnit::Points, "-72pt"),
            ("0cm", LengthUnit::Points, "0pt"),
            ("3pt", LengthUnit::Points, "3pt"),
        ] {
            assert_eq!(length(from).convert_to(unit), Ok(length(to)), "{from} in {unit}");
        }
    }

    #[test]
    fn a_length_times_a_factor_keeps_its_unit_and_rounds_once() {
        assert_eq!(length("4cm").times(I48F16::from_num(1.25)), Ok(length("5cm")));
        assert_eq!(length("2em").times(I48F16::from_num(0.5)), Ok(length("1em")));
        // 3 scaled points times 0.5: 1.5, a half rounded away from zero.
        let three = Length::new(I48F16::from_bits(3), LengthUnit::Points);
        assert_eq!(three.times(I48F16::from_num(0.5)).expect("a product").amount.to_bits(), 2);
        let huge = Length::new(I48F16::MAX, LengthUnit::Points);
        assert_eq!(huge.times(I48F16::from_num(2)), Err(LengthConversionError::Overflow));
    }

    #[test]
    fn a_conversion_rounds_once_to_the_nearest_scaled_point() {
        // 1pt = 127/360 mm = 0.352777… mm: 23119.64… scaled points, 23120.
        let mm = length("1pt").convert_to(LengthUnit::Millimeters).expect("a conversion");
        assert_eq!(mm.amount.to_bits(), 23120);
        // 1mm = 360/127 pt = 2.834645… pt: 185771.33… scaled points, 185771.
        let pt = length("1mm").convert_to(LengthUnit::Points).expect("a conversion");
        assert_eq!(pt.amount.to_bits(), 185771);
        assert_eq!(length("-1mm").convert_to(LengthUnit::Points).expect("a conversion")
            .amount.to_bits(), -185771);
        // A half rounds away from zero: 72 scaled points of a point are one
        // scaled point of an inch, so 36 are exactly half of one.
        let half = Length::new(I48F16::from_bits(36), LengthUnit::Points);
        assert_eq!(half.convert_to(LengthUnit::Inches).expect("a conversion").amount.to_bits(), 1);
        let minus_half = Length::new(I48F16::from_bits(-36), LengthUnit::Points);
        assert_eq!(minus_half.convert_to(LengthUnit::Inches).expect("a conversion")
            .amount.to_bits(), -1);
        let under_half = Length::new(I48F16::from_bits(35), LengthUnit::Points);
        assert_eq!(under_half.convert_to(LengthUnit::Inches).expect("a conversion")
            .amount.to_bits(), 0);
    }

    #[test]
    fn a_round_trip_through_a_coarser_unit_comes_back_within_its_rounding() {
        for text in ["10pt", "3.3mm", "12.7cm", "0.1in", "-7.25pt", "1000mm"] {
            let start = length(text);
            for unit in [LengthUnit::Points, LengthUnit::Millimeters,
                LengthUnit::Centimeters, LengthUnit::Inches] {
                let back = start.convert_to(unit).and_then(|there| there.convert_to(start.unit))
                    .expect("a physical round trip");
                // Half a scaled point of the intermediate unit is at most 36
                // scaled points of the start's unit; the way back rounds once more.
                let off = (back.amount.to_bits() - start.amount.to_bits()).abs();
                assert!(off <= 73, "{text} through {unit}: {back}");
            }
            // Through its own unit, exactly.
            assert_eq!(start.convert_to(start.unit), Ok(start));
        }
    }

    #[test]
    fn a_tab_or_an_em_converts_to_nothing() {
        for unit in [LengthUnit::Tabs, LengthUnit::Ems] {
            assert!(!unit.is_physical());
            let value = Length::new(I48F16::from_num(2), unit);
            assert_eq!(value.convert_to(LengthUnit::Points),
                Err(LengthConversionError::NotPhysical { unit }));
            assert_eq!(length("1pt").convert_to(unit),
                Err(LengthConversionError::NotPhysical { unit }));
            assert_eq!(length("1pt").scaled(&value, &length("1pt"), LengthUnit::Points),
                Err(LengthConversionError::NotPhysical { unit }));
            assert_eq!(length("1pt").scaled(&length("1pt"), &value, LengthUnit::Points),
                Err(LengthConversionError::NotPhysical { unit }));
            assert_eq!(value.scaled(&length("1pt"), &length("1pt"), LengthUnit::Points),
                Err(LengthConversionError::NotPhysical { unit }));
        }
        for unit in [LengthUnit::Points, LengthUnit::Millimeters, LengthUnit::Centimeters,
            LengthUnit::Inches] {
            assert!(unit.is_physical());
        }
    }

    #[test]
    fn a_scaling_keeps_the_proportions_in_the_given_unit() {
        // 340pt × 170pt shown 5cm wide is 2.5cm high, and 2.5cm high 5cm wide.
        let (width, height) = (length("340pt"), length("170pt"));
        let given = length("5cm");
        assert_eq!(height.scaled(&given, &width, given.unit), Ok(length("2.5cm")));
        let given = length("2.5cm");
        assert_eq!(width.scaled(&given, &height, given.unit), Ok(length("5cm")));
        // Across units, exactly: 1in wide, 36pt high, shown 2in wide: 1in high.
        assert_eq!(length("36pt").scaled(&length("2in"), &length("1in"), LengthUnit::Inches),
            Ok(length("1in")));
        assert_eq!(length("1pt").scaled(&length("1pt"), &length("0pt"), LengthUnit::Points),
            Err(LengthConversionError::ZeroDenominator));
        // A result beyond the amount's range is refused, not wrapped.
        let huge = Length::new(I48F16::MAX, LengthUnit::Inches);
        assert_eq!(huge.scaled(&huge, &length("1pt"), LengthUnit::Points),
            Err(LengthConversionError::Overflow));
        assert_eq!(huge.convert_to(LengthUnit::Points), Err(LengthConversionError::Overflow));
    }

    #[test]
    fn dots_at_a_resolution_give_points() {
        assert_eq!(Length::from_dots(300, I48F16::from_num(150)), Ok(length("144pt")));
        assert_eq!(Length::from_dots(96, I48F16::from_num(96)), Ok(length("72pt")));
        assert_eq!(Length::from_dots(0, I48F16::from_num(96)), Ok(length("0pt")));
        // 100 dots at 96 per inch: 75pt exactly; at 7 per inch, 1028.571428…pt.
        assert_eq!(Length::from_dots(100, I48F16::from_num(96)), Ok(length("75pt")));
        let odd = Length::from_dots(100, I48F16::from_num(7)).expect("a size");
        assert_eq!(odd.amount.to_bits(), (7200_i64 * 65536 + 3) / 7);
        assert_eq!(Length::from_dots(1, I48F16::ZERO), Err(LengthConversionError::ZeroDenominator));
        assert_eq!(Length::from_dots(u64::MAX, I48F16::from_bits(1)),
            Err(LengthConversionError::Overflow));
    }
}
