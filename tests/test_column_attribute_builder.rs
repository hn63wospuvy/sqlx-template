/// Test #[column("name")] attribute with builder pattern (tp_select_builder, tp_update_builder)
/// Verifies that builder-generated SQL uses custom column names.

use sqlx_template::SqliteTemplate;
use sqlx::{FromRow, SqlitePool};

// Struct with #[column] and builder pattern
#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("employees")]
#[tp_select_builder(
    with_department = "q_dept = :department",
    with_name_search = "q_name LIKE :pattern$String",
    with_role_and_dept = "q_role = :role$String AND q_dept = :dept$String"
)]
#[tp_update_builder]
pub struct Employee {
    #[auto]
    pub id: i32,
    #[column("q_name")]
    pub name: String,
    #[column("q_dept")]
    pub department: String,
    #[column("q_role")]
    pub role: String,
    pub salary: i32,
    pub active: bool,
}

// Struct with #[column] and field-based builder conditions
#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("tasks")]
#[tp_select_builder]
pub struct Task {
    #[auto]
    pub id: i32,
    #[column("task_title")]
    pub title: String,
    #[column("task_status")]
    pub status: String,
    pub priority: i32,
    #[column("assigned_to")]
    pub assignee: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing #[column] with Builder Pattern ===\n");

    let pool = SqlitePool::connect(":memory:").await?;

    // Create tables with actual DB column names
    sqlx::query(
        r#"
        CREATE TABLE employees (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            q_name TEXT NOT NULL,
            q_dept TEXT NOT NULL,
            q_role TEXT NOT NULL,
            salary INTEGER NOT NULL,
            active BOOLEAN NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE tasks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_title TEXT NOT NULL,
            task_status TEXT NOT NULL,
            priority INTEGER NOT NULL,
            assigned_to TEXT NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // Insert employees
    let employees = vec![
        Employee { id: 0, name: "Alice".to_string(), department: "Engineering".to_string(), role: "Senior".to_string(), salary: 120000, active: true },
        Employee { id: 0, name: "Bob".to_string(), department: "Engineering".to_string(), role: "Junior".to_string(), salary: 80000, active: true },
        Employee { id: 0, name: "Charlie".to_string(), department: "Marketing".to_string(), role: "Senior".to_string(), salary: 100000, active: true },
        Employee { id: 0, name: "Diana".to_string(), department: "Marketing".to_string(), role: "Junior".to_string(), salary: 70000, active: false },
        Employee { id: 0, name: "Eve".to_string(), department: "Engineering".to_string(), role: "Lead".to_string(), salary: 150000, active: true },
    ];
    for emp in &employees {
        Employee::insert(emp, &pool).await?;
    }
    println!("✅ Inserted 5 employees using INSERT with #[column]");

    // Insert tasks
    let tasks = vec![
        Task { id: 0, title: "Fix bug".to_string(), status: "open".to_string(), priority: 1, assignee: "Alice".to_string() },
        Task { id: 0, title: "Add feature".to_string(), status: "in_progress".to_string(), priority: 2, assignee: "Bob".to_string() },
        Task { id: 0, title: "Write docs".to_string(), status: "open".to_string(), priority: 3, assignee: "Alice".to_string() },
        Task { id: 0, title: "Review PR".to_string(), status: "done".to_string(), priority: 1, assignee: "Charlie".to_string() },
    ];
    for task in &tasks {
        Task::insert(task, &pool).await?;
    }
    println!("✅ Inserted 4 tasks using INSERT with #[column]");

    // ========= Test SELECT builder with custom conditions =========
    println!("\n--- Testing SELECT builder with custom conditions ---");

    // with_department - uses q_dept column
    let eng_employees = Employee::builder_select()
        .with_department("Engineering")?
        .find_all(&pool)
        .await?;
    assert_eq!(eng_employees.len(), 3, "Should find 3 Engineering employees");
    for emp in &eng_employees {
        assert_eq!(emp.department, "Engineering");
    }
    println!("✅ with_department correctly queries q_dept column");

    // with_name_search - uses q_name column with LIKE
    let alice_results = Employee::builder_select()
        .with_name_search("Ali%")?
        .find_all(&pool)
        .await?;
    assert_eq!(alice_results.len(), 1);
    assert_eq!(alice_results[0].name, "Alice");
    println!("✅ with_name_search correctly queries q_name column with LIKE");

    // with_role_and_dept - uses both q_role and q_dept columns
    let senior_eng = Employee::builder_select()
        .with_role_and_dept("Senior", "Engineering")?
        .find_all(&pool)
        .await?;
    assert_eq!(senior_eng.len(), 1);
    assert_eq!(senior_eng[0].name, "Alice");
    println!("✅ with_role_and_dept correctly queries q_role and q_dept columns");

    // Chaining custom conditions
    let eng_seniors = Employee::builder_select()
        .with_department("Engineering")?
        .with_name_search("E%")?
        .find_all(&pool)
        .await?;
    assert_eq!(eng_seniors.len(), 1);
    assert_eq!(eng_seniors[0].name, "Eve");
    println!("✅ Chaining custom conditions works with #[column]");

    // ========= Test SELECT builder with field-based conditions =========
    println!("\n--- Testing SELECT builder with field-based conditions ---");

    // Field-based builder should generate methods that reference actual column names
    let open_tasks = Task::builder_select()
        .status("open")?
        .find_all(&pool)
        .await?;
    assert_eq!(open_tasks.len(), 2);
    println!("✅ Field-based status() queries task_status column");

    let alice_tasks = Task::builder_select()
        .assignee("Alice")?
        .find_all(&pool)
        .await?;
    assert_eq!(alice_tasks.len(), 2);
    println!("✅ Field-based assignee() queries assigned_to column");

    let high_priority_open = Task::builder_select()
        .status("open")?
        .priority(&1)?
        .find_all(&pool)
        .await?;
    assert_eq!(high_priority_open.len(), 1);
    assert_eq!(high_priority_open[0].title, "Fix bug");
    println!("✅ Chaining field-based conditions works with #[column]");

    // ========= Test UPDATE builder =========
    println!("\n--- Testing UPDATE builder with #[column] ---");

    // Update salary for employee by id
    Employee::builder_update()
        .on_salary(&130000)?
        .by_id(&1)?
        .execute(&pool)
        .await?;
    
    let alice = Employee::builder_select()
        .with_name_search("Alice")?
        .find_all(&pool)
        .await?;
    assert_eq!(alice[0].salary, 130000);
    println!("✅ UPDATE builder on_salary works");

    // Verify using raw SQL that actual DB columns are correct
    let row: (String, String, String, i32) = sqlx::query_as(
        "SELECT q_name, q_dept, q_role, salary FROM employees WHERE id = 1"
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(row.0, "Alice");
    assert_eq!(row.1, "Engineering");
    assert_eq!(row.2, "Senior");
    assert_eq!(row.3, 130000);
    println!("✅ Raw SQL confirms correct column names after UPDATE");

    // ========= Test count and page with builder =========
    println!("\n--- Testing count and page with builder ---");

    let count = Employee::builder_select()
        .with_department("Engineering")?
        .count(&pool)
        .await?;
    assert_eq!(count, 3);
    println!("✅ builder count() works with #[column] conditions");

    let (page, total) = Employee::builder_select()
        .with_department("Engineering")?
        .find_page((0i64, 2i32, true), &pool)
        .await?;
    assert!(page.len() <= 2);
    assert_eq!(total.unwrap(), 3);
    println!("✅ builder find_page() works with #[column] conditions");

    // ========= Test find_one with builder =========
    println!("\n--- Testing find_one with builder ---");

    let one = Employee::builder_select()
        .with_name_search("Bob")?
        .find_one(&pool)
        .await?;
    assert!(one.is_some());
    assert_eq!(one.unwrap().name, "Bob");
    println!("✅ builder find_one() works with #[column]");

    let none = Employee::builder_select()
        .with_name_search("Nonexistent%")?
        .find_one(&pool)
        .await?;
    assert!(none.is_none());
    println!("✅ builder find_one() returns None for non-existent with #[column]");

    println!("\n🎉 All builder pattern #[column] tests passed!");

    Ok(())
}
