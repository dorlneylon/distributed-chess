fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .type_attribute(
            "GameState",
            "#[derive(serde::Serialize, serde::Deserialize)]",
        )
        .type_attribute("Board", "#[derive(serde::Serialize, serde::Deserialize)]")
        .type_attribute("Piece", "#[derive(serde::Serialize, serde::Deserialize)]")
        .type_attribute("Color", "#[derive(serde::Serialize, serde::Deserialize)]")
        .type_attribute(
            "Location",
            "#[derive(serde::Serialize, serde::Deserialize)]",
        )
        .type_attribute("Row", "#[derive(serde::Serialize, serde::Deserialize)]")
        .type_attribute("Cell", "#[derive(serde::Serialize, serde::Deserialize)]")
        .type_attribute(
            "StateRequest",
            "#[derive(serde::Deserialize, serde::Serialize)]",
        )
        .type_attribute(
            "StartRequest",
            "#[derive(serde::Deserialize, serde::Serialize)]",
        )
        .type_attribute(
            "Transaction",
            "#[derive(serde::Serialize, serde::Deserialize)]",
        )
        .type_attribute(
            "Position",
            "#[derive(serde::Serialize, serde::Deserialize)]",
        )
        .compile(&["proto/game.proto", "proto/query.proto"], &["proto"])?;

    Ok(())
}
