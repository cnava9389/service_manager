use std::fmt;

#[non_exhaustive]
#[warn(non_camel_case_types)]
pub enum ServerError{
    TEST,
    NONE,
    TAKEN,
    MISSING_DATA,
    INVALID_JSON,
    INVALID_DATA,
    INVALID_ARG,
    INVALID_FILE,
    CONNECTION,
    NO_VALUE,
    FAILED_READ,
    FAILED_WRITE,
    INCOMPLETE_OPERATION,
    INCOMPATIBLE_DATA_TYPES,
}

impl fmt::Display for ServerError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "Error occured sorry! {}", self)// user facing
    }
}

impl fmt::Debug for ServerError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt,"{{ file: {}, line: {} }}",file!(),line!())// programmer facing
    }
}

impl std::error::Error for ServerError {}

impl ServerError {
    pub fn produce_error(&self) -> &str {
        match self {
            ServerError::TEST => {
                //println!("Error: ");
                "Error: "
            },
            ServerError::NONE => {
                //println!("Error: None");
                "Error: None"
            },
            ServerError::TAKEN => {
                //println!("Error: Taken");
                "Error: Taken"
            },
            ServerError::MISSING_DATA => {
                //println!("Error: Missing Data");
                "Error: Missing Data"
            },
            ServerError::INVALID_JSON => {
                //println!("Error: Invalid Json");
                "Error: Invalid Json"
            },
            ServerError::INVALID_DATA => {
                //println!("Error: Invalid Data");
                "Error: Invalid Data"
            },
            ServerError::INVALID_ARG => {
                //println!("Error: Invalid Arg");
                "Error: Invalid Arg"
            },
            ServerError::CONNECTION => {
                //println!("Error: Connection");
                "Error: Connection"
            },
            ServerError::NO_VALUE => {
                //println!("Error: No Value");
                "Error: No Value"
            },
            ServerError::INVALID_FILE => {
                //println!("Error: Invalid File");
                "Error: Invalid File"
            }
            ServerError::FAILED_READ => {
                //println!("Error: Failed to Read");
                "Error: Failed to Read"
            }
            ServerError::FAILED_WRITE => {
                //println!("Error: Failed to Write");
                "Error: Failed to Write"
            }
            ServerError::INCOMPLETE_OPERATION => {
                //println!("Error: Incomplete operation");
                "Error: Incomplete operation"
            }
            ServerError::INCOMPATIBLE_DATA_TYPES => {
                //println!("Error: Incompatible data types");
                "Error: Incompatible data types"
            }
        }
    }

}