#[derive(Debug)]
pub enum Error {
    DuplicatedArgument { arg: &'static str },
    MissingRequiredArgument { arg: &'static str },
    UnexpectedArgument { arg: String },

    InvalidValue { arg: &'static str, value: String },
    MissingValue { arg: &'static str },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicatedArgument { arg } => write!(f, "duplicated argument {arg}"),
            Self::MissingRequiredArgument { arg } => write!(f, "missing required argument {arg}"),
            Self::UnexpectedArgument { arg } => write!(f, "unexpected argument: {arg}"),

            Self::InvalidValue { arg, value } => {
                write!(f, "invalid value for argument {arg}: {value}")
            }
            Self::MissingValue { arg } => write!(f, "missing value for argument {arg}"),
        }
    }
}

impl std::error::Error for Error {}

macro_rules! args {
    // Internal rules used to handle optional default values
    (@unwrap $arg:literal, $field:ident $(,)?) => {
        $field.ok_or_else(|| $crate::args::Error::MissingRequiredArgument { arg: $arg })?
    };
    (@unwrap $arg:literal, $field:ident, $default:literal) => {
        $field.unwrap_or_else(|| $default.parse().unwrap())
    };

    (
        pub struct $struct:ident {
            $($arg:literal => $(#[arg(default_value = $default:literal)])? pub $field:ident: $ty:ty),+ $(,)?
        }
    ) => {
        #[derive(::std::fmt::Debug)]
        pub struct $struct {
            $(pub $field: $ty,)+
        }

        impl $struct {
            pub fn parse() -> ::std::result::Result<Self, $crate::args::Error> {
                $(let mut $field: ::std::option::Option<$ty> = None;)+

                // The first argument is the path of the executable.
                let mut args_iter = ::std::env::args().skip(1);
                while let ::std::option::Option::Some(arg) = args_iter.next() {
                    match arg.as_str() {
                        $($arg => {
                            if $field.is_some() {
                                return ::std::result::Result::Err($crate::args::Error::DuplicatedArgument { arg: $arg });
                            }
                            let raw_value = args_iter
                                .next()
                                .ok_or_else(|| $crate::args::Error::MissingValue { arg: $arg })?;
                            let value = raw_value.parse().map_err(|_| $crate::args::Error::InvalidValue {
                                arg: $arg,
                                value: raw_value,
                            })?;
                            $field = ::std::option::Option::Some(value);
                        })+
                        _ => return ::std::result::Result::Err($crate::args::Error::UnexpectedArgument { arg }),
                    }
                }

                ::std::result::Result::Ok($struct {
                    $($field: $crate::args::args!(@unwrap $arg, $field, $($default)?),)+
                })
            }
        }
    }
}

pub(crate) use args;
