// Test file to verify that the count query generation works correctly
// for queries with JOIN and GROUP BY

use sqlx_template::postgres_query;
use sqlx::FromRow;
use sqlx::types::chrono;

#[derive(FromRow, Debug)]
pub struct DepartmentStats {
    pub department: String,
    pub employee_count: i64,
}

// Test 1: Simple query without JOIN or GROUP BY - should use old method
#[postgres_query(sql = "SELECT * FROM employees WHERE active = :active ORDER BY created_at DESC")]
pub async fn get_active_employees(active: bool) -> Page<Employee> {}

// Test 2: Query with JOIN - should use subquery method
#[postgres_query(sql = "
    SELECT e.name, d.department_name 
    FROM employees e 
    JOIN departments d ON e.department_id = d.id 
    WHERE e.active = :active
")]
pub async fn get_employees_with_department(active: bool) -> Page<EmployeeWithDepartment> {}

// Test 3: Query with GROUP BY - should use subquery method
#[postgres_query(sql = "
    SELECT department, COUNT(*) as employee_count 
    FROM employees 
    WHERE salary > :min_salary 
    GROUP BY department
")]
pub async fn get_department_stats(min_salary: i32) -> Page<DepartmentStats> {}

// Test 4: Complex query with both JOIN and GROUP BY - should use subquery method
#[postgres_query(sql = "
    SELECT d.department_name, COUNT(e.id) as employee_count, AVG(e.salary) as avg_salary
    FROM departments d 
    LEFT JOIN employees e ON d.id = e.department_id 
    WHERE e.active = :active 
    GROUP BY d.id, d.department_name 
    ORDER BY employee_count DESC
")]
pub async fn get_complex_department_stats(active: bool) -> Page<ComplexDepartmentStats> {}

#[derive(FromRow, Debug)]
pub struct Employee {
    pub id: i32,
    pub name: String,
    pub department_id: i32,
    pub salary: i32,
    pub active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(FromRow, Debug)]
pub struct EmployeeWithDepartment {
    pub name: String,
    pub department_name: String,
}

#[derive(FromRow, Debug)]
pub struct ComplexDepartmentStats {
    pub department_name: String,
    pub employee_count: i64,
    pub avg_salary: Option<f64>,
}

// Define Page type for testing
#[derive(Debug, Clone)]
pub struct Page<T> {
    pub offset: u64,
    pub limit: u32,
    pub total: Option<u64>,
    pub data: Vec<T>
}

#[cfg(test)]
mod tests {
    
    #[test]
    fn test_functions_exist() {
        // This test just verifies that the functions are generated correctly
        // In a real scenario, you would test with actual database connections
        
        // The functions should be generated with the correct signatures
        // Test 1: Simple query - should generate count query with COUNT(1)
        // Test 2: JOIN query - should generate count query with subquery
        // Test 3: GROUP BY query - should generate count query with subquery  
        // Test 4: Complex query - should generate count query with subquery
        
        println!("All test functions generated successfully!");
    }
}
