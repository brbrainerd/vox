use crate::commands::extras::ludus::LudusContext;
use anyhow::Result;
use owo_colors::OwoColorize;
use vox_gamify::{LudusProfile, db as ludus_db, shop};

/// List available items in the shop.
pub async fn shop_list() -> Result<()> {
    let ctx = LudusContext::load().await?;
    let codex = &ctx.db;
    let user_id = &ctx.user_id;
    let profile = ludus_db::get_profile(codex, user_id)
        .await?
        .unwrap_or_else(|| LudusProfile::new_default(user_id));

    let items = shop::default_shop_items();
    let mode_mult = 1.0; // In a real app, this comes from config_gate/VoxConfig

    println!("{}", "╔══════════════════════════════════╗".bright_cyan());
    println!("{}", "║       💎 Crystal Shop           ║".bright_cyan());
    println!("{}", "╚══════════════════════════════════╝".bright_cyan());
    println!();
    println!(
        "  Your Balance: {} 💎",
        profile.crystals.to_string().bright_yellow()
    );
    println!();

    println!("  {:<5} {:<30} {:>10}", "ID", "Item", "Cost");
    println!("  {}", "─".repeat(47).dimmed());

    for (i, item) in items.iter().enumerate() {
        let cost = item.effective_cost(mode_mult);
        let id = (i + 1).to_string();

        println!(
            "  {:<5} {:<30} {:>10} 💎",
            id.bright_white(),
            item.name().bright_white(),
            cost.to_string().bright_yellow()
        );
    }

    println!();
    println!(
        "  Use {} to buy an item.",
        "vox ludus shop buy --item-id <ID>".bright_green()
    );

    Ok(())
}

/// Purchase an item from the shop.
pub async fn shop_buy(item_id: &str) -> Result<()> {
    let ctx = LudusContext::load().await?;
    let codex = &ctx.db;
    let user_id = &ctx.user_id;
    let mut profile = ludus_db::get_profile(codex, user_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Profile not found"))?;

    let items = shop::default_shop_items();
    let index = item_id
        .parse::<usize>()
        .map(|i| i.saturating_sub(1))
        .map_err(|_| anyhow::anyhow!("Invalid item ID"))?;

    let item = items
        .get(index)
        .ok_or_else(|| anyhow::anyhow!("Item not found"))?;

    let mode_mult = 1.0;
    let mut abilities = vec![]; // Simplified for now

    let result = shop::purchase(&mut profile, item, mode_mult, &mut abilities);

    if result.success {
        ludus_db::upsert_profile(codex, &profile).await?;
        println!("  ✅ {}", result.message.bright_green());
        println!(
            "  Remaining: {} 💎",
            result.crystals_remaining.to_string().bright_yellow()
        );
    } else {
        println!("  ❌ {}", result.message.bright_red());
    }

    Ok(())
}
