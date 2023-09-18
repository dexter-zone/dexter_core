#[macro_export]
macro_rules! add_wasm_execute_msg {
    ($self:ident, $contract_addr:expr, $wasm_msg:expr, $funds: expr) => {
        $self.push(CosmosMsg::Wasm(
            cosmwasm_std::WasmMsg::Execute {
                contract_addr: $contract_addr.to_string(),
                msg: to_binary(&$wasm_msg)?,
                funds: $funds,
            })
        );
    }
}