//! Seed data: initial resources, recipes, and markets.
//!
//! Run once on first launch to populate the game world with
//! raw materials, production recipes, and market entries.

use rust_decimal_macros::dec;
use sqlx::PgPool;
use uuid::Uuid;

/// Seed all static game data.
pub async fn seed_all(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Check if already seeded.
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM resources")
        .fetch_one(pool)
        .await?;
    if count.0 > 0 {
        tracing::info!("Database already seeded, skipping");
        return Ok(());
    }

    tracing::info!("Seeding initial game data...");

    // ── Resources ───────────────────────────────────────────────────
    struct Res {
        id: Uuid,
        slug: &'static str,
        name: &'static str,
        category: &'static str,
        base_price: rust_decimal::Decimal,
        spawn_rate: rust_decimal::Decimal,
    }

    let resources = vec![
        // Raw materials (spawned by NPC)
        Res { id: Uuid::new_v4(), slug: "copper",   name: "Copper Ore",   category: "raw_material", base_price: dec!(50),   spawn_rate: dec!(100) },
        Res { id: Uuid::new_v4(), slug: "silicon",  name: "Silicon",      category: "raw_material", base_price: dec!(60),   spawn_rate: dec!(80) },
        Res { id: Uuid::new_v4(), slug: "lithium",  name: "Lithium",      category: "raw_material", base_price: dec!(120),  spawn_rate: dec!(40) },
        Res { id: Uuid::new_v4(), slug: "iron",     name: "Iron Ore",     category: "raw_material", base_price: dec!(40),   spawn_rate: dec!(120) },
        Res { id: Uuid::new_v4(), slug: "plastic",  name: "Plastic",      category: "raw_material", base_price: dec!(30),   spawn_rate: dec!(150) },
        Res { id: Uuid::new_v4(), slug: "oil",      name: "Crude Oil",    category: "raw_material", base_price: dec!(80),   spawn_rate: dec!(60) },
        // Components (manufactured)
        Res { id: Uuid::new_v4(), slug: "steel",        name: "Steel",         category: "component", base_price: dec!(100),  spawn_rate: dec!(0) },
        Res { id: Uuid::new_v4(), slug: "electronics",  name: "Electronics",   category: "component", base_price: dec!(200),  spawn_rate: dec!(0) },
        Res { id: Uuid::new_v4(), slug: "battery_pack", name: "Battery Pack",  category: "component", base_price: dec!(350),  spawn_rate: dec!(0) },
        Res { id: Uuid::new_v4(), slug: "fuel",         name: "Refined Fuel",  category: "component", base_price: dec!(150),  spawn_rate: dec!(0) },
        // Finished goods
        Res { id: Uuid::new_v4(), slug: "machines",      name: "Industrial Machines", category: "finished_good", base_price: dec!(800),  spawn_rate: dec!(0) },
        Res { id: Uuid::new_v4(), slug: "vehicles",      name: "Vehicles",            category: "finished_good", base_price: dec!(1200), spawn_rate: dec!(0) },
        Res { id: Uuid::new_v4(), slug: "consumer_tech", name: "Consumer Tech",       category: "finished_good", base_price: dec!(600),  spawn_rate: dec!(0) },
        // Luxury goods
        Res { id: Uuid::new_v4(), slug: "luxury_goods",  name: "Luxury Goods", category: "luxury", base_price: dec!(2000), spawn_rate: dec!(0) },
    ];

    for r in &resources {
        sqlx::query(
            "INSERT INTO resources (id, slug, name, category, base_price, spawn_rate) VALUES ($1, $2, $3, $4, $5, $6)"
        )
        .bind(r.id).bind(r.slug).bind(r.name).bind(r.category)
        .bind(r.base_price).bind(r.spawn_rate)
        .execute(pool).await?;

        // Create a market for each resource.
        sqlx::query(
            "INSERT INTO markets (id, resource_id, last_price, ema_price) VALUES ($1, $2, $3, $4)"
        )
        .bind(Uuid::new_v4()).bind(r.id).bind(r.base_price).bind(r.base_price)
        .execute(pool).await?;
    }

    // ── Helper: find resource id by slug ────────────────────────────
    let rid = |slug: &str| -> Uuid {
        resources.iter().find(|r| r.slug == slug).unwrap().id
    };

    // ── Recipes ─────────────────────────────────────────────────────
    struct RecipeSeed {
        slug: &'static str,
        name: &'static str,
        ticks: i32,
        min_tech: i32,
        workers: i32,
        inputs: Vec<(&'static str, rust_decimal::Decimal)>,
        outputs: Vec<(&'static str, rust_decimal::Decimal)>,
    }

    let recipes = vec![
        RecipeSeed {
            slug: "smelt_steel", name: "Smelt Steel", ticks: 2, min_tech: 0, workers: 5,
            inputs: vec![("iron", dec!(3))],
            outputs: vec![("steel", dec!(1))],
        },
        RecipeSeed {
            slug: "make_electronics", name: "Manufacture Electronics", ticks: 3, min_tech: 1, workers: 8,
            inputs: vec![("copper", dec!(2)), ("silicon", dec!(1))],
            outputs: vec![("electronics", dec!(1))],
        },
        RecipeSeed {
            slug: "make_battery", name: "Assemble Battery Pack", ticks: 4, min_tech: 1, workers: 6,
            inputs: vec![("lithium", dec!(2)), ("copper", dec!(1)), ("plastic", dec!(1))],
            outputs: vec![("battery_pack", dec!(1))],
        },
        RecipeSeed {
            slug: "refine_fuel", name: "Refine Fuel", ticks: 2, min_tech: 0, workers: 4,
            inputs: vec![("oil", dec!(3))],
            outputs: vec![("fuel", dec!(2))],
        },
        RecipeSeed {
            slug: "build_machines", name: "Build Machines", ticks: 5, min_tech: 2, workers: 12,
            inputs: vec![("steel", dec!(3)), ("electronics", dec!(2))],
            outputs: vec![("machines", dec!(1))],
        },
        RecipeSeed {
            slug: "build_vehicles", name: "Build Vehicles", ticks: 6, min_tech: 2, workers: 15,
            inputs: vec![("steel", dec!(4)), ("electronics", dec!(2)), ("battery_pack", dec!(1)), ("fuel", dec!(2))],
            outputs: vec![("vehicles", dec!(1))],
        },
        RecipeSeed {
            slug: "make_consumer_tech", name: "Make Consumer Tech", ticks: 3, min_tech: 1, workers: 10,
            inputs: vec![("electronics", dec!(2)), ("plastic", dec!(2)), ("battery_pack", dec!(1))],
            outputs: vec![("consumer_tech", dec!(2))],
        },
        RecipeSeed {
            slug: "make_luxury", name: "Produce Luxury Goods", ticks: 8, min_tech: 3, workers: 20,
            inputs: vec![("consumer_tech", dec!(2)), ("vehicles", dec!(1)), ("machines", dec!(1))],
            outputs: vec![("luxury_goods", dec!(1))],
        },
    ];

    for recipe in &recipes {
        let recipe_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO recipes (id, slug, name, ticks_required, min_tech_level, workers_required)
             VALUES ($1, $2, $3, $4, $5, $6)"
        )
        .bind(recipe_id).bind(recipe.slug).bind(recipe.name)
        .bind(recipe.ticks).bind(recipe.min_tech).bind(recipe.workers)
        .execute(pool).await?;

        for (slug, qty) in &recipe.inputs {
            sqlx::query(
                "INSERT INTO recipe_items (id, recipe_id, resource_id, resource_slug, quantity, direction)
                 VALUES ($1, $2, $3, $4, $5, 'input')"
            )
            .bind(Uuid::new_v4()).bind(recipe_id).bind(rid(slug)).bind(*slug).bind(*qty)
            .execute(pool).await?;
        }

        for (slug, qty) in &recipe.outputs {
            sqlx::query(
                "INSERT INTO recipe_items (id, recipe_id, resource_id, resource_slug, quantity, direction)
                 VALUES ($1, $2, $3, $4, $5, 'output')"
            )
            .bind(Uuid::new_v4()).bind(recipe_id).bind(rid(slug)).bind(*slug).bind(*qty)
            .execute(pool).await?;
        }
    }

    tracing::info!("Seed complete: {} resources, {} recipes", resources.len(), recipes.len());
    Ok(())
}
