fn ntm_register_bomb_command(commands:&mut Vec<horizon_client::mod_api::NativeCommand>){
    commands.push(horizon_client::mod_api::NativeCommand::op("ntmitem [args:text]",2));
    commands.push(horizon_client::mod_api::NativeCommand::op("ntmbomba [args:text]",2));
    commands.push(horizon_client::mod_api::NativeCommand::op("ntmledger",2));
    // CommandRadiation: CommandBase's level, 4.
    commands.push(horizon_client::mod_api::NativeCommand::op("ntmrad [args:text]",4));
    // The RBMK dials (Java's `/gamerule dial...`) and the explicit rebuilding of standing columns (T02, D5).
    commands.push(horizon_client::mod_api::NativeCommand::op("ntmrbmk [args:text]",2));
    // The in-game proofs' hooks (glue `test_hook`): registered only with NTM_TEST_HOOKS=1, as the save's failpoints.
    if std::env::var_os("NTM_TEST_HOOKS").is_some() {
        commands.push(horizon_client::mod_api::NativeCommand::op("ntmtest [args:text]",2));
    }
}
