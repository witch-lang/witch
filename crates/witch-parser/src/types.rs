use core::hash::{Hash, Hasher};
use core::mem::discriminant;
use std::collections::HashMap;
use witch_runtime::value::Value;

use crate::ast::{Ast, Operator};

#[derive(PartialEq, Clone, Debug)]
pub enum TypeDecl {
    Struct {
        generics: HashMap<String, Type>,
        fields: Vec<(String, Type)>,
        methods: Vec<(String, Ast)>,
    },
    Interface {
        generics: Vec<(String, Type)>,
        properties: HashMap<String, Type>,
    },
    Enum {
        generics: HashMap<String, Type>,
        variants: Vec<EnumVariant>,
    },
}

#[derive(Debug, PartialEq, Clone)]
pub enum Index {
    String(String),
    Int(i32),
}

#[derive(Debug, PartialEq, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub discriminant: usize,
    pub types: Option<Vec<Type>>,
}

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum Type {
    /// Void signifies nothing.
    /// It's equivalent to Unit ().
    Void,

    /// A UTF-8 encoded string.
    String,

    /// True or false
    Bool,

    /// The Any type is always equal to all types.
    /// Very dangerous!
    Any,

    /// Signed 8-bit integer
    I8,

    /// Unsigned 8-bit integer
    U8,

    /// Signed 16-bit integer
    I16,

    /// Unsigned 16-bit integer
    U16,

    /// Signed 32-bit integer
    I32,

    /// Unsigned 32-bit integer
    U32,

    /// Signed 64-bit integer
    I64,

    /// Unsigned 64-bit integer
    U64,

    /// Signed 128-bit integer
    I128,

    /// Unsigned 128-bit integer
    U128,

    /// Signed integer of target architecture max int size
    Isize,

    /// Unsigned integer of target architecture max int size
    Usize,

    /// ASCII char
    Char,

    /// 32-bit floating point number
    F32,

    /// 64-bit floating point number
    F64,

    /// A function
    Function {
        /// List of argument types
        args: Vec<Self>,

        /// The function return type
        returns: Box<Self>,

        /// Whether the function is variadic, i.e. if it takes a variable number of arguments
        is_variadic: bool,

        /// A hashmap of defined type variables, e.g. <T, U>(arg: U) -> T {}
        generics: HashMap<String, Self>,
    },

    /// A list of some type
    List(Box<Self>),

    /// A custom struct type
    Struct {
        /// Name of the struct type, or
        /// None of its an anonymous struct
        name: Option<String>,

        /// Since a Struct is represented by a List at runtime,
        /// fields need to be represented as a vec here to preserve
        /// their order
        fields: Vec<(String, Self)>,

        /// A map of methods: <Name, (Type, functions/vtable index)>
        methods: HashMap<String, (Self, usize)>,

        /// A hashmap of defined type variables, e.g. [T, U]
        generics: HashMap<String, Self>,
    },

    /// An interface that other types can be compared against
    Interface {
        /// Name of the interface
        name: String,

        /// A map of properties to compare against: <Name, Type>
        properties: HashMap<String, Self>,

        /// A hashmap of defined type variables, e.g. [T, U]
        generics: Vec<(String, Self)>,
    },

    /// An enum is simply a list of its variants.
    /// You can't instantiate an enum without a variant.
    Enum(Vec<EnumVariant>),

    /// An enum variant holds its name, discriminant, associated data types,
    /// as well as any generics used
    EnumVariant(EnumVariant),

    /// A variable referencing to a different type (generic or custom made)
    TypeVar { name: String, inner: Vec<Type> },

    /// A variable referencing a Value
    Var(String),

    /// A type that merges multiple types into one.
    /// This allows us to compare type T to U: InterfaceOne + InterfaceTwo, etc
    Intersection(Vec<Type>),

    /// An unknown type is one that we haven't yet inferred, or are unable to do so
    Unknown,
}

impl Eq for Type {}

impl PartialEq for Type {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // If either side is Any, return true
            (Type::Any, _) => true,
            (_, Type::Any) => true,

            // Lists are equal based on their contained type
            (Type::List(v1), Type::List(v2)) => v1 == v2,

            // Functions are compared on their arguments and return types
            (
                Type::Function {
                    args: fa1,
                    returns: fr1,
                    ..
                },
                Type::Function {
                    args: fa2,
                    returns: fr2,
                    ..
                },
            ) => {
                if fa1.len() != fa2.len() {
                    return false;
                }
                for (f1, f2) in fa1.iter().zip(fa2.iter()) {
                    if f1 != f2 {
                        return false;
                    }
                }

                fr1 == fr2
            }

            // Named structs are nominally typed, i.e. we can compare by name.
            // For anonymous structs we make a structural comparison.
            (
                Type::Struct {
                    name: n1,
                    fields: f1,
                    methods: m1,
                    ..
                },
                Type::Struct {
                    name: n2,
                    fields: f2,
                    methods: m2,
                    ..
                },
            ) => {
                // Compare names, if any
                if let (Some(n1), Some(n2)) = (n1, n2) {
                    return n1 == n2;
                }

                // Compare fields, if any
                for (key, fieldtype1) in f1.iter().enumerate() {
                    if let Some(fieldtype2) = f2.get(key) {
                        if fieldtype1 != fieldtype2 {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                // Compare methods, if any
                for (key, (methodtype1, _)) in m1 {
                    if let Some((methodtype2, _)) = m2.get(key) {
                        if methodtype1 != methodtype2 {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                true
            }

            // Interface == Struct: Duck typed (as long as Struct has matching methods or fields, is a match)
            (
                Type::Interface { properties, .. },
                Type::Struct {
                    fields, methods, ..
                },
            ) => {
                for (name, ty) in properties.iter() {
                    // If the property exists as a method
                    if let Some((method_type, _)) = methods.get(name) {
                        // Check its type
                        if ty != method_type {
                            return false;
                        }
                    // If it's not a method, check the fields
                    } else if fields
                        .iter()
                        .filter(|(field_name, field_type)| field_name == name && field_type == ty)
                        .collect::<Vec<_>>()
                        .len()
                        != 1
                    {
                        return false;
                    // Property does not have a corresponding method or field
                    } else {
                        return false;
                    }
                }
                true
            }

            // Checks whether an Enum Variant is of type Enum.
            // E.g. MyEnum.One == MyEnum
            (Type::Enum(variants), Type::EnumVariant(variant)) => variants.contains(variant),
            (Type::EnumVariant(variant), Type::Enum(variants)) => variants.contains(variant),

            // For primitive types, just match on the enum discriminant
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

impl Hash for Type {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // TODO recursively do this for function types.
        match self {
            Type::TypeVar { name, inner } => {
                name.hash(state);
                for i in inner {
                    i.hash(state);
                }
            }
            _ => discriminant(self).hash(state),
        }
    }
}

impl From<&Value> for Type {
    fn from(value: &Value) -> Type {
        match value {
            Value::Usize(_) => Type::Usize,
            Value::Isize(_) => Type::Isize,
            Value::List(vec) => {
                if !vec.is_empty() {
                    Type::List(Box::new((&vec[0]).into()))
                } else {
                    Type::List(Box::new(Type::Any))
                }
            }
            Value::String(_) => Type::String,
            Value::Function(_) => Type::Unknown,
            x => todo!("{:?}", x),
        }
    }
}

impl Type {
    pub fn allowed_infix_operators(&self, rhs: &Type) -> Vec<Operator> {
        match (self, rhs) {
            (Type::Usize, Type::Usize) => vec![
                Operator::Add,
                Operator::Sub,
                Operator::Div,
                Operator::Mul,
                Operator::Mod,
                Operator::Lt,
            ],
            (Type::String, Type::Usize) => vec![Operator::Mul],
            _ => vec![],
        }
    }

    pub fn is_numeric(&self) -> bool {
        use Type::*;
        matches!(
            self,
            Usize | Isize | U8 | I8 | U16 | I16 | U32 | I32 | U64 | I64 | U128 | I128 | F32 | F64
        )
    }

    pub fn from_str(str: &str, inner: Vec<Type>) -> Type {
        match &*str.to_lowercase() {
            "void" => Type::Void,
            "bool" => Type::Bool,
            "string" => Type::String,
            "any" => Type::Any,
            "i8" => Type::I8,
            "u8" => Type::U8,
            "i16" => Type::I16,
            "u16" => Type::U16,
            "i32" => Type::I32,
            "u32" => Type::U32,
            "i64" => Type::I64,
            "u64" => Type::U64,
            "i128" => Type::I128,
            "u128" => Type::U128,
            "isize" => Type::Isize,
            "usize" => Type::Usize,

            _ => Type::TypeVar {
                name: str.to_string(),
                inner,
            },
        }
    }
}
