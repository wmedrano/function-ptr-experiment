use std::ops::Deref;
use std::rc::Rc;

use compact_str::CompactString;

use crate::func::Func;
use crate::Result;

/// A native Rust function callable from the VM.
#[derive(Clone)]
pub struct NativeFunc(pub fn(&[Val]) -> Result<Val>);

impl PartialEq for NativeFunc {
    fn eq(&self, other: &Self) -> bool {
        self.0 as usize == other.0 as usize
    }
}

impl std::fmt::Debug for NativeFunc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NativeFunc(..)")
    }
}

/// A value in the interpreter.
#[derive(Clone, PartialEq)]
pub enum Val {
    Bool(bool),
    Int(i64),
    String(RcString),
    Symbol(Symbol),
    Func(Func),
    NativeFunc(NativeFunc),
}

/// Implements `From<$type>` for [`Val`], converting into the given variant.
macro_rules! impl_from_for_val {
    ($type:ty => $variant:ident) => {
        impl From<$type> for Val {
            fn from(v: $type) -> Val {
                Val::$variant(v.into())
            }
        }
    };
}

impl Val {
    pub(crate) fn type_name(&self) -> &'static str {
        match self {
            Val::Bool(_) => "Bool",
            Val::Int(_) => "Int",
            Val::String(_) => "Str",
            Val::Symbol(_) => "Symbol",
            Val::Func(_) => "Func",
            Val::NativeFunc(_) => "NativeFunc",
        }
    }

    pub fn is_truthy(&self) -> bool {
        !matches!(self, Val::Bool(false))
    }
}

impl_from_for_val!(bool => Bool);
impl_from_for_val!(i64 => Int);
impl_from_for_val!(RcString => String);
impl_from_for_val!(Symbol => Symbol);
impl_from_for_val!(Func => Func);
impl_from_for_val!(NativeFunc => NativeFunc);

impl From<&str> for Val {
    fn from(v: &str) -> Val {
        Val::String(RcString::new(v))
    }
}

impl From<String> for Val {
    fn from(v: String) -> Val {
        Val::String(RcString::new(v))
    }
}

/// An immutable, reference-counted string.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RcString(Rc<CompactString>);

impl RcString {
    pub fn new(s: impl Into<CompactString>) -> RcString {
        RcString(Rc::new(s.into()))
    }
}

impl Deref for RcString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

impl std::fmt::Display for RcString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl std::fmt::Debug for RcString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0.as_str())
    }
}

/// The maximum size of a symbol in bytes.
///
/// This constant is the maximum allowable size before `Val` becomes too large.
pub const MAX_SYMBOL_LEN: usize = 15;

/// A symbol, backed by up to `MAX_SYMBOL_LEN` bytes of UTF-8 data.
#[derive(Copy, Clone, PartialEq)]
pub struct Symbol([u8; MAX_SYMBOL_LEN]);

impl Symbol {
    /// Create a `Symbol` from a string slice. Returns an error if `s` is longer than
    /// `MAX_SYMBOL_LEN` bytes.
    pub fn new(s: &str) -> crate::Result<Symbol> {
        let bytes = s.as_bytes();
        if bytes.len() > MAX_SYMBOL_LEN {
            return Err(crate::Error::SymbolTooLong { len: bytes.len() });
        }
        let mut data = [0u8; MAX_SYMBOL_LEN];
        data[..bytes.len()].copy_from_slice(bytes);
        Ok(Symbol(data))
    }

    pub(crate) fn as_str(&self) -> &str {
        let len = self.0.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        // SAFETY: `self.0` is always initialized from a valid `&str`, so its bytes are valid UTF-8.
        unsafe { std::str::from_utf8_unchecked(&self.0[..len]) }
    }
}

impl std::fmt::Debug for Val {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Val::Bool(v) => write!(f, "Bool({v:?})"),
            Val::Int(v) => write!(f, "Int({v:?})"),
            Val::String(v) => write!(f, "Str({v:?})"),
            Val::Symbol(v) => write!(f, "Symbol({v:?})"),
            Val::Func(_) => write!(f, "Func(..)"),
            Val::NativeFunc(_) => write!(f, "NativeFunc(..)"),
        }
    }
}

impl std::fmt::Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::fmt::Debug for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Symbol({:?})", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn val_with_variants_is_within_sixteen_bytes() {
        assert!(
            std::mem::size_of::<Val>() <= 16,
            "Val size is {}, expected <= 16",
            std::mem::size_of::<Val>()
        );
    }

    #[test]
    fn str_with_content_returns_valid_metadata() {
        let s = RcString::new("hello");
        assert_eq!(s.len(), 5);
        assert_eq!(&*s, "hello");

        let v: Val = "world".into();
        assert_eq!(v.type_name(), "Str");
        assert_eq!(format!("{v:?}"), "Str(\"world\")");
    }

    #[test]
    fn symbol_with_valid_length_succeeds() {
        let s = Symbol::new("short").unwrap();
        assert_eq!(s.as_str(), "short");
        assert_eq!(s.as_str().len(), 5);
    }

    #[test]
    fn symbol_with_max_length_succeeds() {
        let max_str = "a".repeat(MAX_SYMBOL_LEN);
        let s = Symbol::new(&max_str).unwrap();
        assert_eq!(s.as_str(), max_str);
        assert_eq!(s.as_str().len(), MAX_SYMBOL_LEN);
    }

    #[test]
    fn symbol_with_exceeding_length_fails() {
        let too_long = "a".repeat(MAX_SYMBOL_LEN + 1);
        let res = Symbol::new(&too_long);
        assert_eq!(
            res,
            Err(crate::Error::SymbolTooLong {
                len: MAX_SYMBOL_LEN + 1
            })
        );
    }
}

#[cfg(test)]
mod native_tests {
    use super::*;
    use crate::Vm;

    fn add_native(args: &[Val]) -> crate::Result<Val> {
        if args.len() != 2 {
            return Err(crate::Error::WrongArgCount {
                expected: 2,
                got: args.len(),
            });
        }
        let (Val::Int(a), Val::Int(b)) = (&args[0], &args[1]) else {
            return Err(crate::Error::WrongType {
                expected: "Int",
                got: "other",
            });
        };
        Ok(Val::Int(a + b))
    }

    #[test]
    fn native_func_with_addition_returns_sum() {
        let mut vm = Vm::new();
        let nf = Val::NativeFunc(NativeFunc(add_native));

        let result = vm.run_with_func(nf, vec![Val::Int(5), Val::Int(7)]);
        assert_eq!(result, Ok(Val::Int(12)));
    }
}
