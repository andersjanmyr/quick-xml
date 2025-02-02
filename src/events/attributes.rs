//! Xml Attributes module
//!
//! Provides an iterator over attributes key/value pairs

use crate::errors::{Error, Result as XmlResult};
use crate::escape::{do_unescape, escape};
use crate::reader::{is_whitespace, Reader};
use crate::utils::{write_byte_string, write_cow_string, Bytes};
use std::fmt::{self, Debug, Display, Formatter};
use std::iter::FusedIterator;
use std::{borrow::Cow, collections::HashMap, io::BufRead, ops::Range};

/// A struct representing a key/value XML attribute.
///
/// Field `value` stores raw bytes, possibly containing escape-sequences. Most users will likely
/// want to access the value using one of the [`unescaped_value`] and [`unescape_and_decode_value`]
/// functions.
///
/// [`unescaped_value`]: #method.unescaped_value
/// [`unescape_and_decode_value`]: #method.unescape_and_decode_value
#[derive(Clone, PartialEq)]
pub struct Attribute<'a> {
    /// The key to uniquely define the attribute.
    ///
    /// If [`Attributes::with_checks`] is turned off, the key might not be unique.
    ///
    /// [`Attributes::with_checks`]: struct.Attributes.html#method.with_checks
    pub key: &'a [u8],
    /// The raw value of the attribute.
    pub value: Cow<'a, [u8]>,
}

impl<'a> Attribute<'a> {
    /// Returns the unescaped value.
    ///
    /// This is normally the value you are interested in. Escape sequences such as `&gt;` are
    /// replaced with their unescaped equivalents such as `>`.
    ///
    /// This will allocate if the value contains any escape sequences.
    ///
    /// See also [`unescaped_value_with_custom_entities()`](#method.unescaped_value_with_custom_entities)
    pub fn unescaped_value(&self) -> XmlResult<Cow<[u8]>> {
        self.make_unescaped_value(None)
    }

    /// Returns the unescaped value, using custom entities.
    ///
    /// This is normally the value you are interested in. Escape sequences such as `&gt;` are
    /// replaced with their unescaped equivalents such as `>`.
    /// Additional entities can be provided in `custom_entities`.
    ///
    /// This will allocate if the value contains any escape sequences.
    ///
    /// See also [`unescaped_value()`](#method.unescaped_value)
    ///
    /// # Pre-condition
    ///
    /// The keys and values of `custom_entities`, if any, must be valid UTF-8.
    pub fn unescaped_value_with_custom_entities(
        &self,
        custom_entities: &HashMap<Vec<u8>, Vec<u8>>,
    ) -> XmlResult<Cow<[u8]>> {
        self.make_unescaped_value(Some(custom_entities))
    }

    fn make_unescaped_value(
        &self,
        custom_entities: Option<&HashMap<Vec<u8>, Vec<u8>>>,
    ) -> XmlResult<Cow<[u8]>> {
        do_unescape(&*self.value, custom_entities).map_err(Error::EscapeError)
    }

    /// Decode then unescapes the value
    ///
    /// This allocates a `String` in all cases. For performance reasons it might be a better idea to
    /// instead use one of:
    ///
    /// * [`Reader::decode()`], as it only allocates when the decoding can't be performed otherwise.
    /// * [`unescaped_value()`], as it doesn't allocate when no escape sequences are used.
    ///
    /// [`unescaped_value()`]: #method.unescaped_value
    /// [`Reader::decode()`]: ../../reader/struct.Reader.html#method.decode
    pub fn unescape_and_decode_value<B: BufRead>(&self, reader: &Reader<B>) -> XmlResult<String> {
        self.do_unescape_and_decode_value(reader, None)
    }

    /// Decode then unescapes the value with custom entities
    ///
    /// This allocates a `String` in all cases. For performance reasons it might be a better idea to
    /// instead use one of:
    ///
    /// * [`Reader::decode()`], as it only allocates when the decoding can't be performed otherwise.
    /// * [`unescaped_value_with_custom_entities()`], as it doesn't allocate when no escape sequences are used.
    ///
    /// [`unescaped_value_with_custom_entities()`]: #method.unescaped_value_with_custom_entities
    /// [`Reader::decode()`]: ../../reader/struct.Reader.html#method.decode
    ///
    /// # Pre-condition
    ///
    /// The keys and values of `custom_entities`, if any, must be valid UTF-8.
    pub fn unescape_and_decode_value_with_custom_entities<B: BufRead>(
        &self,
        reader: &Reader<B>,
        custom_entities: &HashMap<Vec<u8>, Vec<u8>>,
    ) -> XmlResult<String> {
        self.do_unescape_and_decode_value(reader, Some(custom_entities))
    }

    /// The keys and values of `custom_entities`, if any, must be valid UTF-8.
    #[cfg(feature = "encoding")]
    fn do_unescape_and_decode_value<B: BufRead>(
        &self,
        reader: &Reader<B>,
        custom_entities: Option<&HashMap<Vec<u8>, Vec<u8>>>,
    ) -> XmlResult<String> {
        let decoded = reader.decode(&*self.value);
        let unescaped =
            do_unescape(decoded.as_bytes(), custom_entities).map_err(Error::EscapeError)?;
        String::from_utf8(unescaped.into_owned()).map_err(|e| Error::Utf8(e.utf8_error()))
    }

    #[cfg(not(feature = "encoding"))]
    fn do_unescape_and_decode_value<B: BufRead>(
        &self,
        reader: &Reader<B>,
        custom_entities: Option<&HashMap<Vec<u8>, Vec<u8>>>,
    ) -> XmlResult<String> {
        let decoded = reader.decode(&*self.value)?;
        let unescaped =
            do_unescape(decoded.as_bytes(), custom_entities).map_err(Error::EscapeError)?;
        String::from_utf8(unescaped.into_owned()).map_err(|e| Error::Utf8(e.utf8_error()))
    }

    /// helper method to unescape then decode self using the reader encoding
    /// but without BOM (Byte order mark)
    ///
    /// for performance reasons (could avoid allocating a `String`),
    /// it might be wiser to manually use
    /// 1. BytesText::unescaped()
    /// 2. Reader::decode(...)
    #[cfg(feature = "encoding")]
    pub fn unescape_and_decode_without_bom<B: BufRead>(
        &self,
        reader: &mut Reader<B>,
    ) -> XmlResult<String> {
        self.do_unescape_and_decode_without_bom(reader, None)
    }

    /// helper method to unescape then decode self using the reader encoding
    /// but without BOM (Byte order mark)
    ///
    /// for performance reasons (could avoid allocating a `String`),
    /// it might be wiser to manually use
    /// 1. BytesText::unescaped()
    /// 2. Reader::decode(...)
    #[cfg(not(feature = "encoding"))]
    pub fn unescape_and_decode_without_bom<B: BufRead>(
        &self,
        reader: &Reader<B>,
    ) -> XmlResult<String> {
        self.do_unescape_and_decode_without_bom(reader, None)
    }

    /// helper method to unescape then decode self using the reader encoding with custom entities
    /// but without BOM (Byte order mark)
    ///
    /// for performance reasons (could avoid allocating a `String`),
    /// it might be wiser to manually use
    /// 1. BytesText::unescaped()
    /// 2. Reader::decode(...)
    ///
    /// # Pre-condition
    ///
    /// The keys and values of `custom_entities`, if any, must be valid UTF-8.
    #[cfg(feature = "encoding")]
    pub fn unescape_and_decode_without_bom_with_custom_entities<B: BufRead>(
        &self,
        reader: &mut Reader<B>,
        custom_entities: &HashMap<Vec<u8>, Vec<u8>>,
    ) -> XmlResult<String> {
        self.do_unescape_and_decode_without_bom(reader, Some(custom_entities))
    }

    /// helper method to unescape then decode self using the reader encoding with custom entities
    /// but without BOM (Byte order mark)
    ///
    /// for performance reasons (could avoid allocating a `String`),
    /// it might be wiser to manually use
    /// 1. BytesText::unescaped()
    /// 2. Reader::decode(...)
    ///
    /// # Pre-condition
    ///
    /// The keys and values of `custom_entities`, if any, must be valid UTF-8.
    #[cfg(not(feature = "encoding"))]
    pub fn unescape_and_decode_without_bom_with_custom_entities<B: BufRead>(
        &self,
        reader: &Reader<B>,
        custom_entities: &HashMap<Vec<u8>, Vec<u8>>,
    ) -> XmlResult<String> {
        self.do_unescape_and_decode_without_bom(reader, Some(custom_entities))
    }

    #[cfg(feature = "encoding")]
    fn do_unescape_and_decode_without_bom<B: BufRead>(
        &self,
        reader: &mut Reader<B>,
        custom_entities: Option<&HashMap<Vec<u8>, Vec<u8>>>,
    ) -> XmlResult<String> {
        let decoded = reader.decode_without_bom(&*self.value);
        let unescaped =
            do_unescape(decoded.as_bytes(), custom_entities).map_err(Error::EscapeError)?;
        String::from_utf8(unescaped.into_owned()).map_err(|e| Error::Utf8(e.utf8_error()))
    }

    #[cfg(not(feature = "encoding"))]
    fn do_unescape_and_decode_without_bom<B: BufRead>(
        &self,
        reader: &Reader<B>,
        custom_entities: Option<&HashMap<Vec<u8>, Vec<u8>>>,
    ) -> XmlResult<String> {
        let decoded = reader.decode_without_bom(&*self.value)?;
        let unescaped =
            do_unescape(decoded.as_bytes(), custom_entities).map_err(Error::EscapeError)?;
        String::from_utf8(unescaped.into_owned()).map_err(|e| Error::Utf8(e.utf8_error()))
    }
}

impl<'a> Debug for Attribute<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Attribute {{ key: ")?;
        write_byte_string(f, self.key)?;
        write!(f, ", value: ")?;
        write_cow_string(f, &self.value)?;
        write!(f, " }}")
    }
}

impl<'a> From<(&'a [u8], &'a [u8])> for Attribute<'a> {
    /// Creates new attribute from raw bytes.
    /// Does not apply any transformation to both key and value.
    ///
    /// # Examples
    ///
    /// ```
    /// # use pretty_assertions::assert_eq;
    /// use quick_xml::events::attributes::Attribute;
    ///
    /// let features = Attribute::from(("features".as_bytes(), "Bells &amp; whistles".as_bytes()));
    /// assert_eq!(features.value, "Bells &amp; whistles".as_bytes());
    /// ```
    fn from(val: (&'a [u8], &'a [u8])) -> Attribute<'a> {
        Attribute {
            key: val.0,
            value: Cow::from(val.1),
        }
    }
}

impl<'a> From<(&'a str, &'a str)> for Attribute<'a> {
    /// Creates new attribute from text representation.
    /// Key is stored as-is, but the value will be escaped.
    ///
    /// # Examples
    ///
    /// ```
    /// # use pretty_assertions::assert_eq;
    /// use quick_xml::events::attributes::Attribute;
    ///
    /// let features = Attribute::from(("features", "Bells & whistles"));
    /// assert_eq!(features.value, "Bells &amp; whistles".as_bytes());
    /// ```
    fn from(val: (&'a str, &'a str)) -> Attribute<'a> {
        Attribute {
            key: val.0.as_bytes(),
            value: escape(val.1.as_bytes()),
        }
    }
}

impl<'a> From<Attr<&'a [u8]>> for Attribute<'a> {
    #[inline]
    fn from(attr: Attr<&'a [u8]>) -> Self {
        Self {
            key: attr.key(),
            value: Cow::Borrowed(attr.value()),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Iterator over XML attributes.
///
/// Yields `Result<Attribute>`. An `Err` will be yielded if an attribute is malformed or duplicated.
/// The duplicate check can be turned off by calling [`with_checks(false)`].
///
/// [`with_checks(false)`]: #method.with_checks
#[derive(Clone, Debug)]
pub struct Attributes<'a> {
    /// slice of `Element` corresponding to attributes
    bytes: &'a [u8],
    /// Iterator state, independent from the actual source of bytes
    state: IterState,
}

impl<'a> Attributes<'a> {
    /// Creates a new attribute iterator from a buffer.
    pub fn new(buf: &'a [u8], pos: usize) -> Self {
        Self {
            bytes: buf,
            state: IterState::new(pos, false),
        }
    }

    /// Creates a new attribute iterator from a buffer, allowing HTML attribute syntax.
    pub fn html(buf: &'a [u8], pos: usize) -> Self {
        Self {
            bytes: buf,
            state: IterState::new(pos, true),
        }
    }

    /// Changes whether attributes should be checked for uniqueness.
    ///
    /// The XML specification requires attribute keys in the same element to be unique. This check
    /// can be disabled to improve performance slightly.
    ///
    /// (`true` by default)
    pub fn with_checks(&mut self, val: bool) -> &mut Attributes<'a> {
        self.state.check_duplicates = val;
        self
    }
}

impl<'a> Iterator for Attributes<'a> {
    type Item = Result<Attribute<'a>, AttrError>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.state.next(self.bytes) {
            None => None,
            Some(Ok(a)) => Some(Ok(a.map(|range| &self.bytes[range]).into())),
            Some(Err(e)) => Some(Err(e)),
        }
    }
}

impl<'a> FusedIterator for Attributes<'a> {}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Errors that can be raised during parsing attributes.
///
/// Recovery position in examples shows the position from which parsing of the
/// next attribute will be attempted.
#[derive(Debug, PartialEq)]
pub enum AttrError {
    /// Attribute key was not followed by `=`, position relative to the start of
    /// the owning tag is provided.
    ///
    /// Example of input that raises this error:
    ///
    /// ```xml
    /// <tag key another="attribute"/>
    /// <!--     ^~~ error position, recovery position (8) -->
    /// ```
    ///
    /// This error can be raised only when the iterator is in XML mode.
    ExpectedEq(usize),
    /// Attribute value was not found after `=`, position relative to the start
    /// of the owning tag is provided.
    ///
    /// Example of input that raises this error:
    ///
    /// ```xml
    /// <tag key = />
    /// <!--       ^~~ error position, recovery position (10) -->
    /// ```
    ///
    /// This error can be returned only for the last attribute in the list,
    /// because otherwise any content after `=` will be threated as a value.
    /// The XML
    ///
    /// ```xml
    /// <tag key = another-key = "value"/>
    /// <!--                   ^ ^- recovery position (24) -->
    /// <!--                   '~~ error position (22) -->
    /// ```
    ///
    /// will be treated as `Attribute { key = b"key", value = b"another-key" }`
    /// and or [`Attribute`] is returned, or [`AttrError::UnquotedValue`] is raised,
    /// depending on the parsing mode.
    ExpectedValue(usize),
    /// Attribute value is not quoted, position relative to the start of the
    /// owning tag is provided.
    ///
    /// Example of input that raises this error:
    ///
    /// ```xml
    /// <tag key = value />
    /// <!--       ^    ^~~ recovery position (15) -->
    /// <!--       '~~ error position (10) -->
    /// ```
    ///
    /// This error can be raised only when the iterator is in XML mode.
    UnquotedValue(usize),
    /// Attribute value was not finished with a matching quote, position relative
    /// to the start of owning tag and a quote is provided. That position is always
    /// a last character in the tag content.
    ///
    /// Example of input that raises this error:
    ///
    /// ```xml
    /// <tag key = "value  />
    /// <tag key = 'value  />
    /// <!--               ^~~ error position, recovery position (18) -->
    /// ```
    ///
    /// This error can be returned only for the last attribute in the list,
    /// because all input was consumed during scanning for a quote.
    ExpectedQuote(usize, u8),
    /// An attribute with the same name was already encountered. Two parameters
    /// define (1) the error position relative to the start of the owning tag
    /// for a new attribute and (2) the start position of a previously encountered
    /// attribute with the same name.
    ///
    /// Example of input that raises this error:
    ///
    /// ```xml
    /// <tag key = 'value'  key="value2" attr3='value3' />
    /// <!-- ^              ^            ^~~ recovery position (32) -->
    /// <!-- |              '~~ error position (19) -->
    /// <!-- '~~ previous position (4) -->
    /// ```
    ///
    /// This error is returned only when [`Attributes::with_checks()`] is set
    /// to `true` (that is default behavior).
    Duplicated(usize, usize),
}

impl Display for AttrError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::ExpectedEq(pos) => write!(
                f,
                r#"position {}: attribute key must be directly followed by `=` or space"#,
                pos
            ),
            Self::ExpectedValue(pos) => write!(
                f,
                r#"position {}: `=` must be followed by an attribute value"#,
                pos
            ),
            Self::UnquotedValue(pos) => write!(
                f,
                r#"position {}: attribute value must be enclosed in `"` or `'`"#,
                pos
            ),
            Self::ExpectedQuote(pos, quote) => write!(
                f,
                r#"position {}: missing closing quote `{}` in attribute value"#,
                pos, *quote as char
            ),
            Self::Duplicated(pos1, pos2) => write!(
                f,
                r#"position {}: duplicated attribute, previous declaration at position {}"#,
                pos1, pos2
            ),
        }
    }
}

impl std::error::Error for AttrError {}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// A struct representing a key/value XML or HTML [attribute].
///
/// [attribute]: https://www.w3.org/TR/xml11/#NT-Attribute
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Attr<T> {
    /// Attribute with value enclosed in double quotes (`"`). Attribute key and
    /// value provided. This is a canonical XML-style attribute.
    DoubleQ(T, T),
    /// Attribute with value enclosed in single quotes (`'`). Attribute key and
    /// value provided. This is an XML-style attribute.
    SingleQ(T, T),
    /// Attribute with value not enclosed in quotes. Attribute key and value
    /// provided. This is HTML-style attribute, it can be returned in HTML-mode
    /// parsing only. In an XML mode [`AttrError::UnquotedValue`] will be raised
    /// instead.
    ///
    /// Attribute value can be invalid according to the [HTML specification],
    /// in particular, it can contain `"`, `'`, `=`, `<`, and <code>&#96;</code>
    /// characters. The absence of the `>` character is nevertheless guaranteed,
    /// since the parser extracts [events] based on them even before the start
    /// of parsing attributes.
    ///
    /// [HTML specification]: https://html.spec.whatwg.org/#unquoted
    /// [events]: crate::events::Event::Start
    Unquoted(T, T),
    /// Attribute without value. Attribute key provided. This is HTML-style attribute,
    /// it can be returned in HTML-mode parsing only. In XML mode
    /// [`AttrError::ExpectedEq`] will be raised instead.
    Empty(T),
}

impl<T> Attr<T> {
    /// Maps an `Attr<T>` to `Attr<U>` by applying a function to a contained key and value.
    #[inline]
    pub fn map<U, F>(self, mut f: F) -> Attr<U>
    where
        F: FnMut(T) -> U,
    {
        match self {
            Attr::DoubleQ(key, value) => Attr::DoubleQ(f(key), f(value)),
            Attr::SingleQ(key, value) => Attr::SingleQ(f(key), f(value)),
            Attr::Empty(key) => Attr::Empty(f(key)),
            Attr::Unquoted(key, value) => Attr::Unquoted(f(key), f(value)),
        }
    }
}

impl<'a> Attr<&'a [u8]> {
    /// Returns the key value
    #[inline]
    pub fn key(&self) -> &'a [u8] {
        match self {
            Attr::DoubleQ(key, _) => key,
            Attr::SingleQ(key, _) => key,
            Attr::Empty(key) => key,
            Attr::Unquoted(key, _) => key,
        }
    }
    /// Returns the attribute value. For [`Self::Empty`] variant an empty slice
    /// is returned according to the [HTML specification].
    ///
    /// [HTML specification]: https://www.w3.org/TR/2012/WD-html-markup-20120329/syntax.html#syntax-attr-empty
    #[inline]
    pub fn value(&self) -> &'a [u8] {
        match self {
            Attr::DoubleQ(_, value) => value,
            Attr::SingleQ(_, value) => value,
            Attr::Empty(_) => &[],
            Attr::Unquoted(_, value) => value,
        }
    }
}

impl<T: AsRef<[u8]>> Debug for Attr<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Attr::DoubleQ(key, value) => f
                .debug_tuple("Attr::DoubleQ")
                .field(&Bytes(key.as_ref()))
                .field(&Bytes(value.as_ref()))
                .finish(),
            Attr::SingleQ(key, value) => f
                .debug_tuple("Attr::SingleQ")
                .field(&Bytes(key.as_ref()))
                .field(&Bytes(value.as_ref()))
                .finish(),
            Attr::Empty(key) => f
                .debug_tuple("Attr::Empty")
                // Comment to prevent formatting and keep style consistent
                .field(&Bytes(key.as_ref()))
                .finish(),
            Attr::Unquoted(key, value) => f
                .debug_tuple("Attr::Unquoted")
                .field(&Bytes(key.as_ref()))
                .field(&Bytes(value.as_ref()))
                .finish(),
        }
    }
}

/// Unpacks attribute key and value into tuple of this two elements.
/// `None` value element is returned only for [`Attr::Empty`] variant.
impl<T> From<Attr<T>> for (T, Option<T>) {
    #[inline]
    fn from(attr: Attr<T>) -> Self {
        match attr {
            Attr::DoubleQ(key, value) => (key, Some(value)),
            Attr::SingleQ(key, value) => (key, Some(value)),
            Attr::Empty(key) => (key, None),
            Attr::Unquoted(key, value) => (key, Some(value)),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

type AttrResult = Result<Attr<Range<usize>>, AttrError>;

#[derive(Clone, Copy, Debug)]
enum State {
    /// Iteration finished, iterator will return `None` to all [`IterState::next`]
    /// requests.
    Done,
    /// The last attribute returned was deserialized successfully. Contains an
    /// offset from which next attribute should be searched.
    Next(usize),
    /// The last attribute returns [`AttrError::UnquotedValue`], offset pointed
    /// to the beginning of the value. Recover should skip a value
    SkipValue(usize),
    /// The last attribute returns [`AttrError::Duplicated`], offset pointed to
    /// the equal (`=`) sign. Recover should skip it and a value
    SkipEqValue(usize),
}

/// External iterator over spans of attribute key and value
#[derive(Clone, Debug)]
pub(crate) struct IterState {
    /// Iteration state that determines what actions should be done before the
    /// actual parsing of the next attribute
    state: State,
    /// If `true`, enables ability to parse unquoted values and key-only (empty)
    /// attributes
    html: bool,
    /// If `true`, checks for duplicate names
    check_duplicates: bool,
    /// If `check_duplicates` is set, contains the ranges of already parsed attribute
    /// names. We store a ranges instead of slices to able to report a previous
    /// attribute position
    keys: Vec<Range<usize>>,
}

impl IterState {
    pub fn new(offset: usize, html: bool) -> Self {
        Self {
            state: State::Next(offset),
            html,
            check_duplicates: true,
            keys: Vec::new(),
        }
    }

    /// Recover from an error that could have been made on a previous step.
    /// Returns an offset from which parsing should continue.
    /// If there no input left, returns `None`.
    fn recover(&self, slice: &[u8]) -> Option<usize> {
        match self.state {
            State::Done => None,
            State::Next(offset) => Some(offset),
            State::SkipValue(offset) => self.skip_value(slice, offset),
            State::SkipEqValue(offset) => self.skip_eq_value(slice, offset),
        }
    }

    /// Skip all characters up to first space symbol or end-of-input
    #[inline]
    fn skip_value(&self, slice: &[u8], offset: usize) -> Option<usize> {
        let mut iter = (offset..).zip(slice[offset..].iter());

        match iter.find(|(_, &b)| is_whitespace(b)) {
            // Input: `    key  =  value `
            //                     |    ^
            //                offset    e
            Some((e, _)) => Some(e),
            // Input: `    key  =  value`
            //                     |    ^
            //                offset    e = len()
            None => None,
        }
    }

    /// Skip all characters up to first space symbol or end-of-input
    #[inline]
    fn skip_eq_value(&self, slice: &[u8], offset: usize) -> Option<usize> {
        let mut iter = (offset..).zip(slice[offset..].iter());

        // Skip all up to the quote and get the quote type
        let quote = match iter.find(|(_, &b)| !is_whitespace(b)) {
            // Input: `    key  =  "`
            //                  |  ^
            //             offset
            Some((_, b'"')) => b'"',
            // Input: `    key  =  '`
            //                  |  ^
            //             offset
            Some((_, b'\'')) => b'\'',

            // Input: `    key  =  x`
            //                  |  ^
            //             offset
            Some((offset, _)) => return self.skip_value(slice, offset),
            // Input: `    key  =  `
            //                  |  ^
            //             offset
            None => return None,
        };

        match iter.find(|(_, &b)| b == quote) {
            // Input: `    key  =  "   "`
            //                         ^
            Some((e, b'"')) => Some(e),
            // Input: `    key  =  '   '`
            //                         ^
            Some((e, _)) => Some(e),

            // Input: `    key  =  "   `
            // Input: `    key  =  '   `
            //                         ^
            // Closing quote not found
            None => None,
        }
    }

    #[inline]
    fn check_for_duplicates(
        &mut self,
        slice: &[u8],
        key: Range<usize>,
    ) -> Result<Range<usize>, AttrError> {
        if self.check_duplicates {
            if let Some(prev) = self
                .keys
                .iter()
                .find(|r| slice[(*r).clone()] == slice[key.clone()])
            {
                return Err(AttrError::Duplicated(key.start, prev.start));
            }
            self.keys.push(key.clone());
        }
        Ok(key)
    }

    /// # Parameters
    ///
    /// - `slice`: content of the tag, used for checking for duplicates
    /// - `key`: Range of key in slice, if iterator in HTML mode
    /// - `offset`: Position of error if iterator in XML mode
    #[inline]
    fn key_only(&mut self, slice: &[u8], key: Range<usize>, offset: usize) -> Option<AttrResult> {
        Some(if self.html {
            self.check_for_duplicates(slice, key).map(Attr::Empty)
        } else {
            Err(AttrError::ExpectedEq(offset))
        })
    }

    #[inline]
    fn double_q(&mut self, key: Range<usize>, value: Range<usize>) -> Option<AttrResult> {
        self.state = State::Next(value.end + 1); // +1 for `"`

        Some(Ok(Attr::DoubleQ(key, value)))
    }

    #[inline]
    fn single_q(&mut self, key: Range<usize>, value: Range<usize>) -> Option<AttrResult> {
        self.state = State::Next(value.end + 1); // +1 for `'`

        Some(Ok(Attr::SingleQ(key, value)))
    }

    pub fn next(&mut self, slice: &[u8]) -> Option<AttrResult> {
        let mut iter = match self.recover(slice) {
            Some(offset) => (offset..).zip(slice[offset..].iter()),
            None => return None,
        };

        // Index where next key started
        let start_key = match iter.find(|(_, &b)| !is_whitespace(b)) {
            // Input: `    key`
            //             ^
            Some((s, _)) => s,
            // Input: `    `
            //             ^
            None => {
                // Because we reach end-of-input, stop iteration on next call
                self.state = State::Done;
                return None;
            }
        };
        // Span of a key
        let (key, offset) = match iter.find(|(_, &b)| b == b'=' || is_whitespace(b)) {
            // Input: `    key=`
            //             |  ^
            //             s  e
            Some((e, b'=')) => (start_key..e, e),

            // Input: `    key `
            //                ^
            Some((e, _)) => match iter.find(|(_, &b)| !is_whitespace(b)) {
                // Input: `    key  =`
                //             |  | ^
                //     start_key  e
                Some((offset, b'=')) => (start_key..e, offset),
                // Input: `    key  x`
                //             |  | ^
                //     start_key  e
                // If HTML-like attributes is allowed, this is the result, otherwise error
                Some((offset, _)) => {
                    // In any case, recovering is not required
                    self.state = State::Next(offset);
                    return self.key_only(slice, start_key..e, offset);
                }
                // Input: `    key  `
                //             |  | ^
                //     start_key  e
                // If HTML-like attributes is allowed, this is the result, otherwise error
                None => {
                    // Because we reach end-of-input, stop iteration on next call
                    self.state = State::Done;
                    return self.key_only(slice, start_key..e, slice.len());
                }
            },

            // Input: `    key`
            //             |  ^
            //             s  e = len()
            // If HTML-like attributes is allowed, this is the result, otherwise error
            None => {
                // Because we reach end-of-input, stop iteration on next call
                self.state = State::Done;
                let e = slice.len();
                return self.key_only(slice, start_key..e, e);
            }
        };

        let key = match self.check_for_duplicates(slice, key) {
            Err(e) => {
                self.state = State::SkipEqValue(offset);
                return Some(Err(e));
            }
            Ok(key) => key,
        };

        ////////////////////////////////////////////////////////////////////////

        // Gets the position of quote and quote type
        let (start_value, quote) = match iter.find(|(_, &b)| !is_whitespace(b)) {
            // Input: `    key  =  "`
            //                     ^
            Some((s, b'"')) => (s + 1, b'"'),
            // Input: `    key  =  '`
            //                     ^
            Some((s, b'\'')) => (s + 1, b'\''),

            // Input: `    key  =  x`
            //                     ^
            // If HTML-like attributes is allowed, this is the start of the value
            Some((s, _)) if self.html => {
                // We do not check validity of attribute value characters as required
                // according to https://html.spec.whatwg.org/#unquoted. It can be done
                // during validation phase
                let end = match iter.find(|(_, &b)| is_whitespace(b)) {
                    // Input: `    key  =  value `
                    //                     |    ^
                    //                     s    e
                    Some((e, _)) => e,
                    // Input: `    key  =  value`
                    //                     |    ^
                    //                     s    e = len()
                    None => slice.len(),
                };
                self.state = State::Next(end);
                return Some(Ok(Attr::Unquoted(key, s..end)));
            }
            // Input: `    key  =  x`
            //                     ^
            Some((s, _)) => {
                self.state = State::SkipValue(s);
                return Some(Err(AttrError::UnquotedValue(s)));
            }

            // Input: `    key  =  `
            //                     ^
            None => {
                // Because we reach end-of-input, stop iteration on next call
                self.state = State::Done;
                return Some(Err(AttrError::ExpectedValue(slice.len())));
            }
        };

        match iter.find(|(_, &b)| b == quote) {
            // Input: `    key  =  "   "`
            //                         ^
            Some((e, b'"')) => self.double_q(key, start_value..e),
            // Input: `    key  =  '   '`
            //                         ^
            Some((e, _)) => self.single_q(key, start_value..e),

            // Input: `    key  =  "   `
            // Input: `    key  =  '   `
            //                         ^
            // Closing quote not found
            None => {
                // Because we reach end-of-input, stop iteration on next call
                self.state = State::Done;
                return Some(Err(AttrError::ExpectedQuote(slice.len(), quote)));
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Checks, how parsing of XML-style attributes works. Each attribute should
/// have a value, enclosed in single or double quotes.
#[cfg(test)]
mod xml {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Checked attribute is the single attribute
    mod single {
        use super::*;
        use pretty_assertions::assert_eq;

        /// Attribute have a value enclosed in single quotes
        #[test]
        fn single_quoted() {
            let mut iter = Attributes::new(br#"tag key='value'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value enclosed in double quotes
        #[test]
        fn double_quoted() {
            let mut iter = Attributes::new(br#"tag key="value""#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value, not enclosed in quotes
        #[test]
        fn unquoted() {
            let mut iter = Attributes::new(br#"tag key=value"#, 3);
            //                                 0       ^ = 8

            assert_eq!(iter.next(), Some(Err(AttrError::UnquotedValue(8))));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Only attribute key is present
        #[test]
        fn key_only() {
            let mut iter = Attributes::new(br#"tag key"#, 3);
            //                                 0      ^ = 7

            assert_eq!(iter.next(), Some(Err(AttrError::ExpectedEq(7))));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key is started with an invalid symbol (a single quote in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_start_invalid() {
            let mut iter = Attributes::new(br#"tag 'key'='value'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"'key'",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key contains an invalid symbol (an ampersand in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_contains_invalid() {
            let mut iter = Attributes::new(br#"tag key&jey='value'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key&jey",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute value is missing after `=`
        #[test]
        fn missed_value() {
            let mut iter = Attributes::new(br#"tag key="#, 3);
            //                                 0       ^ = 8

            assert_eq!(iter.next(), Some(Err(AttrError::ExpectedValue(8))));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }
    }

    /// Checked attribute is the first attribute in the list of many attributes
    mod first {
        use super::*;
        use pretty_assertions::assert_eq;

        /// Attribute have a value enclosed in single quotes
        #[test]
        fn single_quoted() {
            let mut iter = Attributes::new(br#"tag key='value' regular='attribute'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"regular",
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value enclosed in double quotes
        #[test]
        fn double_quoted() {
            let mut iter = Attributes::new(br#"tag key="value" regular='attribute'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"regular",
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value, not enclosed in quotes
        #[test]
        fn unquoted() {
            let mut iter = Attributes::new(br#"tag key=value regular='attribute'"#, 3);
            //                                 0       ^ = 8

            assert_eq!(iter.next(), Some(Err(AttrError::UnquotedValue(8))));
            // check error recovery
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"regular",
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Only attribute key is present
        #[test]
        fn key_only() {
            let mut iter = Attributes::new(br#"tag key regular='attribute'"#, 3);
            //                                 0       ^ = 8

            assert_eq!(iter.next(), Some(Err(AttrError::ExpectedEq(8))));
            // check error recovery
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"regular",
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key is started with an invalid symbol (a single quote in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_start_invalid() {
            let mut iter = Attributes::new(br#"tag 'key'='value' regular='attribute'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"'key'",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"regular",
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key contains an invalid symbol (an ampersand in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_contains_invalid() {
            let mut iter = Attributes::new(br#"tag key&jey='value' regular='attribute'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key&jey",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"regular",
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute value is missing after `=`.
        #[test]
        fn missed_value() {
            let mut iter = Attributes::new(br#"tag key= regular='attribute'"#, 3);
            //                                 0        ^ = 9

            assert_eq!(iter.next(), Some(Err(AttrError::UnquotedValue(9))));
            // Because we do not check validity of keys and values during parsing,
            // "error='recovery'" is considered, as unquoted attribute value and
            // skipped during recovery and iteration finished
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);

            ////////////////////////////////////////////////////////////////////

            let mut iter = Attributes::new(br#"tag key= regular= 'attribute'"#, 3);
            //                                 0        ^ = 9               ^ = 29

            // In that case "regular=" considered as unquoted value
            assert_eq!(iter.next(), Some(Err(AttrError::UnquotedValue(9))));
            // In that case "'attribute'" considered as a key, because we do not check
            // validity of key names
            assert_eq!(iter.next(), Some(Err(AttrError::ExpectedEq(29))));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);

            ////////////////////////////////////////////////////////////////////

            let mut iter = Attributes::new(br#"tag key= regular ='attribute'"#, 3);
            //                                 0        ^ = 9               ^ = 29

            // In that case "regular" considered as unquoted value
            assert_eq!(iter.next(), Some(Err(AttrError::UnquotedValue(9))));
            // In that case "='attribute'" considered as a key, because we do not check
            // validity of key names
            assert_eq!(iter.next(), Some(Err(AttrError::ExpectedEq(29))));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);

            ////////////////////////////////////////////////////////////////////

            let mut iter = Attributes::new(br#"tag key= regular = 'attribute'"#, 3);
            //                                 0        ^ = 9     ^ = 19     ^ = 30

            assert_eq!(iter.next(), Some(Err(AttrError::UnquotedValue(9))));
            // In that case second "=" considered as a key, because we do not check
            // validity of key names
            assert_eq!(iter.next(), Some(Err(AttrError::ExpectedEq(19))));
            // In that case "'attribute'" considered as a key, because we do not check
            // validity of key names
            assert_eq!(iter.next(), Some(Err(AttrError::ExpectedEq(30))));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }
    }

    /// Copy of single, but with additional spaces in markup
    mod sparsed {
        use super::*;
        use pretty_assertions::assert_eq;

        /// Attribute have a value enclosed in single quotes
        #[test]
        fn single_quoted() {
            let mut iter = Attributes::new(br#"tag key = 'value' "#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value enclosed in double quotes
        #[test]
        fn double_quoted() {
            let mut iter = Attributes::new(br#"tag key = "value" "#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value, not enclosed in quotes
        #[test]
        fn unquoted() {
            let mut iter = Attributes::new(br#"tag key = value "#, 3);
            //                                 0         ^ = 10

            assert_eq!(iter.next(), Some(Err(AttrError::UnquotedValue(10))));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Only attribute key is present
        #[test]
        fn key_only() {
            let mut iter = Attributes::new(br#"tag key "#, 3);
            //                                 0       ^ = 8

            assert_eq!(iter.next(), Some(Err(AttrError::ExpectedEq(8))));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key is started with an invalid symbol (a single quote in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_start_invalid() {
            let mut iter = Attributes::new(br#"tag 'key' = 'value' "#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"'key'",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key contains an invalid symbol (an ampersand in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_contains_invalid() {
            let mut iter = Attributes::new(br#"tag key&jey = 'value' "#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key&jey",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute value is missing after `=`
        #[test]
        fn missed_value() {
            let mut iter = Attributes::new(br#"tag key = "#, 3);
            //                                 0         ^ = 10

            assert_eq!(iter.next(), Some(Err(AttrError::ExpectedValue(10))));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }
    }

    /// Checks that duplicated attributes correctly reported and recovering is
    /// possible after that
    mod duplicated {
        use super::*;

        mod with_check {
            use super::*;
            use pretty_assertions::assert_eq;

            /// Attribute have a value enclosed in single quotes
            #[test]
            fn single_quoted() {
                let mut iter = Attributes::new(br#"tag key='value' key='dup' another=''"#, 3);
                //                                 0   ^ = 4       ^ = 16

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"key",
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(iter.next(), Some(Err(AttrError::Duplicated(16, 4))));
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"another",
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Attribute have a value enclosed in double quotes
            #[test]
            fn double_quoted() {
                let mut iter = Attributes::new(br#"tag key='value' key="dup" another=''"#, 3);
                //                                 0   ^ = 4       ^ = 16

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"key",
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(iter.next(), Some(Err(AttrError::Duplicated(16, 4))));
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"another",
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Attribute have a value, not enclosed in quotes
            #[test]
            fn unquoted() {
                let mut iter = Attributes::new(br#"tag key='value' key=dup another=''"#, 3);
                //                                 0   ^ = 4       ^ = 16

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"key",
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(iter.next(), Some(Err(AttrError::Duplicated(16, 4))));
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"another",
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Only attribute key is present
            #[test]
            fn key_only() {
                let mut iter = Attributes::new(br#"tag key='value' key another=''"#, 3);
                //                                 0                   ^ = 20

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"key",
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(iter.next(), Some(Err(AttrError::ExpectedEq(20))));
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"another",
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }
        }

        /// Check for duplicated names is disabled
        mod without_check {
            use super::*;
            use pretty_assertions::assert_eq;

            /// Attribute have a value enclosed in single quotes
            #[test]
            fn single_quoted() {
                let mut iter = Attributes::new(br#"tag key='value' key='dup' another=''"#, 3);
                iter.with_checks(false);

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"key",
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"key",
                        value: Cow::Borrowed(b"dup"),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"another",
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Attribute have a value enclosed in double quotes
            #[test]
            fn double_quoted() {
                let mut iter = Attributes::new(br#"tag key='value' key="dup" another=''"#, 3);
                iter.with_checks(false);

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"key",
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"key",
                        value: Cow::Borrowed(b"dup"),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"another",
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Attribute have a value, not enclosed in quotes
            #[test]
            fn unquoted() {
                let mut iter = Attributes::new(br#"tag key='value' key=dup another=''"#, 3);
                //                                 0                   ^ = 20
                iter.with_checks(false);

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"key",
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(iter.next(), Some(Err(AttrError::UnquotedValue(20))));
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"another",
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Only attribute key is present
            #[test]
            fn key_only() {
                let mut iter = Attributes::new(br#"tag key='value' key another=''"#, 3);
                //                                 0                   ^ = 20
                iter.with_checks(false);

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"key",
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(iter.next(), Some(Err(AttrError::ExpectedEq(20))));
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"another",
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }
        }
    }

    #[test]
    fn mixed_quote() {
        let mut iter = Attributes::new(br#"tag a='a' b = "b" c='cc"cc' d="dd'dd""#, 3);

        assert_eq!(
            iter.next(),
            Some(Ok(Attribute {
                key: b"a",
                value: Cow::Borrowed(b"a"),
            }))
        );
        assert_eq!(
            iter.next(),
            Some(Ok(Attribute {
                key: b"b",
                value: Cow::Borrowed(b"b"),
            }))
        );
        assert_eq!(
            iter.next(),
            Some(Ok(Attribute {
                key: b"c",
                value: Cow::Borrowed(br#"cc"cc"#),
            }))
        );
        assert_eq!(
            iter.next(),
            Some(Ok(Attribute {
                key: b"d",
                value: Cow::Borrowed(b"dd'dd"),
            }))
        );
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
    }
}

/// Checks, how parsing of HTML-style attributes works. Each attribute can be
/// in three forms:
/// - XML-like: have a value, enclosed in single or double quotes
/// - have a value, do not enclosed in quotes
/// - without value, key only
#[cfg(test)]
mod html {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Checked attribute is the single attribute
    mod single {
        use super::*;
        use pretty_assertions::assert_eq;

        /// Attribute have a value enclosed in single quotes
        #[test]
        fn single_quoted() {
            let mut iter = Attributes::html(br#"tag key='value'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value enclosed in double quotes
        #[test]
        fn double_quoted() {
            let mut iter = Attributes::html(br#"tag key="value""#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value, not enclosed in quotes
        #[test]
        fn unquoted() {
            let mut iter = Attributes::html(br#"tag key=value"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Only attribute key is present
        #[test]
        fn key_only() {
            let mut iter = Attributes::html(br#"tag key"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key",
                    value: Cow::Borrowed(&[]),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key is started with an invalid symbol (a single quote in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_start_invalid() {
            let mut iter = Attributes::html(br#"tag 'key'='value'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"'key'",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key contains an invalid symbol (an ampersand in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_contains_invalid() {
            let mut iter = Attributes::html(br#"tag key&jey='value'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key&jey",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute value is missing after `=`
        #[test]
        fn missed_value() {
            let mut iter = Attributes::html(br#"tag key="#, 3);
            //                                  0       ^ = 8

            assert_eq!(iter.next(), Some(Err(AttrError::ExpectedValue(8))));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }
    }

    /// Checked attribute is the first attribute in the list of many attributes
    mod first {
        use super::*;
        use pretty_assertions::assert_eq;

        /// Attribute have a value enclosed in single quotes
        #[test]
        fn single_quoted() {
            let mut iter = Attributes::html(br#"tag key='value' regular='attribute'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"regular",
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value enclosed in double quotes
        #[test]
        fn double_quoted() {
            let mut iter = Attributes::html(br#"tag key="value" regular='attribute'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"regular",
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value, not enclosed in quotes
        #[test]
        fn unquoted() {
            let mut iter = Attributes::html(br#"tag key=value regular='attribute'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"regular",
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Only attribute key is present
        #[test]
        fn key_only() {
            let mut iter = Attributes::html(br#"tag key regular='attribute'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key",
                    value: Cow::Borrowed(&[]),
                }))
            );
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"regular",
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key is started with an invalid symbol (a single quote in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_start_invalid() {
            let mut iter = Attributes::html(br#"tag 'key'='value' regular='attribute'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"'key'",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"regular",
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key contains an invalid symbol (an ampersand in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_contains_invalid() {
            let mut iter = Attributes::html(br#"tag key&jey='value' regular='attribute'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key&jey",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"regular",
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute value is missing after `=`
        #[test]
        fn missed_value() {
            let mut iter = Attributes::html(br#"tag key= regular='attribute'"#, 3);

            // Because we do not check validity of keys and values during parsing,
            // "regular='attribute'" is considered as unquoted attribute value
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key",
                    value: Cow::Borrowed(b"regular='attribute'"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);

            ////////////////////////////////////////////////////////////////////

            let mut iter = Attributes::html(br#"tag key= regular= 'attribute'"#, 3);

            // Because we do not check validity of keys and values during parsing,
            // "regular=" is considered as unquoted attribute value
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key",
                    value: Cow::Borrowed(b"regular="),
                }))
            );
            // Because we do not check validity of keys and values during parsing,
            // "'attribute'" is considered as key-only attribute
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"'attribute'",
                    value: Cow::Borrowed(&[]),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);

            ////////////////////////////////////////////////////////////////////

            let mut iter = Attributes::html(br#"tag key= regular ='attribute'"#, 3);

            // Because we do not check validity of keys and values during parsing,
            // "regular" is considered as unquoted attribute value
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key",
                    value: Cow::Borrowed(b"regular"),
                }))
            );
            // Because we do not check validity of keys and values during parsing,
            // "='attribute'" is considered as key-only attribute
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"='attribute'",
                    value: Cow::Borrowed(&[]),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);

            ////////////////////////////////////////////////////////////////////

            let mut iter = Attributes::html(br#"tag key= regular = 'attribute'"#, 3);
            //                                  0        ^ = 9     ^ = 19     ^ = 30

            // Because we do not check validity of keys and values during parsing,
            // "regular" is considered as unquoted attribute value
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key",
                    value: Cow::Borrowed(b"regular"),
                }))
            );
            // Because we do not check validity of keys and values during parsing,
            // "=" is considered as key-only attribute
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"=",
                    value: Cow::Borrowed(&[]),
                }))
            );
            // Because we do not check validity of keys and values during parsing,
            // "'attribute'" is considered as key-only attribute
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"'attribute'",
                    value: Cow::Borrowed(&[]),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }
    }

    /// Copy of single, but with additional spaces in markup
    mod sparsed {
        use super::*;
        use pretty_assertions::assert_eq;

        /// Attribute have a value enclosed in single quotes
        #[test]
        fn single_quoted() {
            let mut iter = Attributes::html(br#"tag key = 'value' "#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value enclosed in double quotes
        #[test]
        fn double_quoted() {
            let mut iter = Attributes::html(br#"tag key = "value" "#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value, not enclosed in quotes
        #[test]
        fn unquoted() {
            let mut iter = Attributes::html(br#"tag key = value "#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Only attribute key is present
        #[test]
        fn key_only() {
            let mut iter = Attributes::html(br#"tag key "#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key",
                    value: Cow::Borrowed(&[]),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key is started with an invalid symbol (a single quote in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_start_invalid() {
            let mut iter = Attributes::html(br#"tag 'key' = 'value' "#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"'key'",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key contains an invalid symbol (an ampersand in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_contains_invalid() {
            let mut iter = Attributes::html(br#"tag key&jey = 'value' "#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: b"key&jey",
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute value is missing after `=`
        #[test]
        fn missed_value() {
            let mut iter = Attributes::html(br#"tag key = "#, 3);
            //                                  0         ^ = 10

            assert_eq!(iter.next(), Some(Err(AttrError::ExpectedValue(10))));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }
    }

    /// Checks that duplicated attributes correctly reported and recovering is
    /// possible after that
    mod duplicated {
        use super::*;

        mod with_check {
            use super::*;
            use pretty_assertions::assert_eq;

            /// Attribute have a value enclosed in single quotes
            #[test]
            fn single_quoted() {
                let mut iter = Attributes::html(br#"tag key='value' key='dup' another=''"#, 3);
                //                                  0   ^ = 4       ^ = 16

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"key",
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(iter.next(), Some(Err(AttrError::Duplicated(16, 4))));
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"another",
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Attribute have a value enclosed in double quotes
            #[test]
            fn double_quoted() {
                let mut iter = Attributes::html(br#"tag key='value' key="dup" another=''"#, 3);
                //                                  0   ^ = 4       ^ = 16

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"key",
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(iter.next(), Some(Err(AttrError::Duplicated(16, 4))));
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"another",
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Attribute have a value, not enclosed in quotes
            #[test]
            fn unquoted() {
                let mut iter = Attributes::html(br#"tag key='value' key=dup another=''"#, 3);
                //                                  0   ^ = 4       ^ = 16

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"key",
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(iter.next(), Some(Err(AttrError::Duplicated(16, 4))));
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"another",
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Only attribute key is present
            #[test]
            fn key_only() {
                let mut iter = Attributes::html(br#"tag key='value' key another=''"#, 3);
                //                                  0   ^ = 4       ^ = 16

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"key",
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(iter.next(), Some(Err(AttrError::Duplicated(16, 4))));
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"another",
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }
        }

        /// Check for duplicated names is disabled
        mod without_check {
            use super::*;
            use pretty_assertions::assert_eq;

            /// Attribute have a value enclosed in single quotes
            #[test]
            fn single_quoted() {
                let mut iter = Attributes::html(br#"tag key='value' key='dup' another=''"#, 3);
                iter.with_checks(false);

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"key",
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"key",
                        value: Cow::Borrowed(b"dup"),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"another",
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Attribute have a value enclosed in double quotes
            #[test]
            fn double_quoted() {
                let mut iter = Attributes::html(br#"tag key='value' key="dup" another=''"#, 3);
                iter.with_checks(false);

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"key",
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"key",
                        value: Cow::Borrowed(b"dup"),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"another",
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Attribute have a value, not enclosed in quotes
            #[test]
            fn unquoted() {
                let mut iter = Attributes::html(br#"tag key='value' key=dup another=''"#, 3);
                iter.with_checks(false);

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"key",
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"key",
                        value: Cow::Borrowed(b"dup"),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"another",
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Only attribute key is present
            #[test]
            fn key_only() {
                let mut iter = Attributes::html(br#"tag key='value' key another=''"#, 3);
                iter.with_checks(false);

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"key",
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"key",
                        value: Cow::Borrowed(&[]),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: b"another",
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }
        }
    }

    #[test]
    fn mixed_quote() {
        let mut iter = Attributes::html(br#"tag a='a' b = "b" c='cc"cc' d="dd'dd""#, 3);

        assert_eq!(
            iter.next(),
            Some(Ok(Attribute {
                key: b"a",
                value: Cow::Borrowed(b"a"),
            }))
        );
        assert_eq!(
            iter.next(),
            Some(Ok(Attribute {
                key: b"b",
                value: Cow::Borrowed(b"b"),
            }))
        );
        assert_eq!(
            iter.next(),
            Some(Ok(Attribute {
                key: b"c",
                value: Cow::Borrowed(br#"cc"cc"#),
            }))
        );
        assert_eq!(
            iter.next(),
            Some(Ok(Attribute {
                key: b"d",
                value: Cow::Borrowed(b"dd'dd"),
            }))
        );
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
    }
}
