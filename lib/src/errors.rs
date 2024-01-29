#[derive(Debug)]
/// Error enum for the sake of handling stock, orders and stores errors in the system.
/// It groups all the possible errors that can occur in the system.
pub enum Errors {
    FileDoesNotExist,
    ErrorReadingFile,
    CouldNotParse,
    CouldNotReserve,
    LockedError,
    NotEnoughStockError,
    ProductNotFoundError,
    SystemRunFail,
    ConnectionError,
    JoinError,
    ChannelError,
    WriteError,
    ActorMsgError,
    NoActiveStoresError,
    StoreNotConnectedError,
    NoActiveLeader,
    NoStockError,
}

// -------------------- TEST PURPOSE TRAITS --------------------

// Implements the comparison between two Errors for testing purposes
impl PartialEq for Errors {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (
                Errors::StoreNotConnectedError,
                Errors::StoreNotConnectedError
            ) | (Errors::NotEnoughStockError, Errors::NotEnoughStockError)
                | (Errors::ProductNotFoundError, Errors::ProductNotFoundError)
        )
    }
}
