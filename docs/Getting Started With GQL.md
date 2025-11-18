# ISO GQL Guide

Comprehensive set of queries for learning ISO GQL core features.

## Table of Contents
1. [Setup: Schema and Graph Creation](#1-setup-schema-and-graph-creation)
2. [Data Insertion](#2-data-insertion)
3. [Property Updates with SET](#3-property-updates-with-set)
4. [CALL and YIELD](#4-call-and-yield)
5. [Simple Pattern Matching](#5-simple-pattern-matching)
6. [Pattern Matching with WHERE and RETURN](#6-pattern-matching-with-where-and-return)
7. [NEXT Clause](#7-next-clause)
8. [String and Date/Time Functions](#8-string-and-datetime-functions)
9. [ORDER BY Clause](#9-order-by-clause)
10. [GROUP BY and HAVING](#10-group-by-and-having)

---

## Prerequisites

Start the GraphLite REPL:
```bash
# If installed globally
graphlite gql --path ./test_db -u admin -p admin

# Or run from build directory
./target/release/graphlite gql --path ./test_db -u admin -p admin
```

---

## 1. Setup: Schema and Graph Creation

### 1.1 Create Schema
```gql
-- Create a new schema for testing
CREATE SCHEMA /test_schema;
```

**Expected Output:** Success message

### 1.2 Set Session Schema
```gql
-- Set the session to use this schema
SESSION SET SCHEMA /test_schema;
```

**Expected Output:** Schema context updated

### 1.3 Create Graph
```gql
-- Create a graph within the schema
CREATE GRAPH /test_schema/social_network;
```

**Expected Output:** Graph created successfully

### 1.4 Set Session Graph
```gql
-- Set the session to use this graph
SESSION SET GRAPH /test_schema/social_network;
```

**Expected Output:** Graph context updated

### 1.5 Verify Context
```gql
-- Check current session settings
CALL show_session();
```

**Expected Output:** Shows current schema and graph context

---

## 2. Data Insertion - Complete Script (Copy-Paste Ready)

### 2.1 All Data Insertion Commands

**Copy and paste this entire block into the REPL after setting the session graph:**

```gql
-- ============================================
-- COMPLETE DATA INSERTION SCRIPT
-- Copy-paste this entire block into GraphLite REPL
-- ============================================

-- Insert Person nodes
INSERT (:Person {name: 'Alice Johnson', age: 30, email: 'alice@example.com', city: 'New York', joined: '2020-01-15', status: 'active'});
INSERT (:Person {name: 'Bob Smith', age: 25, email: 'bob@example.com', city: 'San Francisco', joined: '2021-03-20', status: 'active'});
INSERT (:Person {name: 'Carol Williams', age: 28, email: 'carol@example.com', city: 'New York', joined: '2020-06-10', status: 'active'});
INSERT (:Person {name: 'David Brown', age: 35, email: 'david@example.com', city: 'Chicago', joined: '2019-11-05', status: 'inactive'});
INSERT (:Person {name: 'Eve Davis', age: 27, email: 'eve@example.com', city: 'San Francisco', joined: '2021-08-12', status: 'active'});
INSERT (:Person {name: 'Frank Miller', age: 32, email: 'frank@example.com', city: 'Boston', joined: '2020-04-18', status: 'active'});

-- Insert Company nodes
INSERT (:Company {name: 'TechCorp', industry: 'Technology', founded: '2010-01-01', employees: 500, revenue: 50000000});
INSERT (:Company {name: 'DataInc', industry: 'Analytics', founded: '2015-06-15', employees: 200, revenue: 20000000});
INSERT (:Company {name: 'CloudSystems', industry: 'Cloud Services', founded: '2012-03-10', employees: 800, revenue: 100000000});

-- Insert Project nodes
INSERT (:Project {name: 'AI Platform', budget: 5000000, start_date: '2023-01-01', status: 'active', priority: 'high'});
INSERT (:Project {name: 'Mobile App', budget: 2000000, start_date: '2023-03-15', status: 'active', priority: 'medium'});
INSERT (:Project {name: 'Data Pipeline', budget: 3000000, start_date: '2022-09-01', status: 'completed', priority: 'high'});
INSERT (:Project {name: 'Security Audit', budget: 500000, start_date: '2023-06-01', status: 'planned', priority: 'low'});

-- Create KNOWS relationships
MATCH (alice:Person {name: 'Alice Johnson'}), (bob:Person {name: 'Bob Smith'}) INSERT (alice)-[:KNOWS {since: '2020-05-10', strength: 'strong'}]->(bob);
MATCH (alice:Person {name: 'Alice Johnson'}), (carol:Person {name: 'Carol Williams'}) INSERT (alice)-[:KNOWS {since: '2020-02-14', strength: 'strong'}]->(carol);
MATCH (bob:Person {name: 'Bob Smith'}), (eve:Person {name: 'Eve Davis'}) INSERT (bob)-[:KNOWS {since: '2021-04-20', strength: 'medium'}]->(eve);
MATCH (carol:Person {name: 'Carol Williams'}), (david:Person {name: 'David Brown'}) INSERT (carol)-[:KNOWS {since: '2019-12-01', strength: 'weak'}]->(david);
MATCH (david:Person {name: 'David Brown'}), (frank:Person {name: 'Frank Miller'}) INSERT (david)-[:KNOWS {since: '2020-01-10', strength: 'medium'}]->(frank);
MATCH (eve:Person {name: 'Eve Davis'}), (frank:Person {name: 'Frank Miller'}) INSERT (eve)-[:KNOWS {since: '2021-09-05', strength: 'strong'}]->(frank);
MATCH (frank:Person {name: 'Frank Miller'}), (alice:Person {name: 'Alice Johnson'}) INSERT (frank)-[:KNOWS {since: '2020-07-22', strength: 'medium'}]->(alice);

-- Create WORKS_AT relationships
MATCH (alice:Person {name: 'Alice Johnson'}), (tech:Company {name: 'TechCorp'}) INSERT (alice)-[:WORKS_AT {role: 'Senior Engineer', since: '2020-02-01', salary: 120000}]->(tech);
MATCH (bob:Person {name: 'Bob Smith'}), (tech:Company {name: 'TechCorp'}) INSERT (bob)-[:WORKS_AT {role: 'Product Manager', since: '2021-04-01', salary: 110000}]->(tech);
MATCH (carol:Person {name: 'Carol Williams'}), (data:Company {name: 'DataInc'}) INSERT (carol)-[:WORKS_AT {role: 'Data Analyst', since: '2020-07-01', salary: 95000}]->(data);
MATCH (david:Person {name: 'David Brown'}), (cloud:Company {name: 'CloudSystems'}) INSERT (david)-[:WORKS_AT {role: 'DevOps Lead', since: '2019-12-01', salary: 130000}]->(cloud);
MATCH (eve:Person {name: 'Eve Davis'}), (tech:Company {name: 'TechCorp'}) INSERT (eve)-[:WORKS_AT {role: 'UX Designer', since: '2021-09-01', salary: 100000}]->(tech);
MATCH (frank:Person {name: 'Frank Miller'}), (data:Company {name: 'DataInc'}) INSERT (frank)-[:WORKS_AT {role: 'Engineering Manager', since: '2020-05-01', salary: 140000}]->(data);

-- Create ASSIGNED_TO relationships
MATCH (alice:Person {name: 'Alice Johnson'}), (ai:Project {name: 'AI Platform'}) INSERT (alice)-[:ASSIGNED_TO {role: 'Tech Lead', allocation: 0.8, start_date: '2023-01-15'}]->(ai);
MATCH (bob:Person {name: 'Bob Smith'}), (mobile:Project {name: 'Mobile App'}) INSERT (bob)-[:ASSIGNED_TO {role: 'Product Owner', allocation: 1.0, start_date: '2023-03-15'}]->(mobile);
MATCH (carol:Person {name: 'Carol Williams'}), (pipeline:Project {name: 'Data Pipeline'}) INSERT (carol)-[:ASSIGNED_TO {role: 'Data Engineer', allocation: 0.5, start_date: '2022-09-01'}]->(pipeline);
MATCH (eve:Person {name: 'Eve Davis'}), (mobile:Project {name: 'Mobile App'}) INSERT (eve)-[:ASSIGNED_TO {role: 'UI/UX Lead', allocation: 0.6, start_date: '2023-03-20'}]->(mobile);
MATCH (frank:Person {name: 'Frank Miller'}), (ai:Project {name: 'AI Platform'}) INSERT (frank)-[:ASSIGNED_TO {role: 'Engineering Manager', allocation: 0.4, start_date: '2023-01-15'}]->(ai);

-- Create SPONSORS relationships
MATCH (tech:Company {name: 'TechCorp'}), (ai:Project {name: 'AI Platform'}) INSERT (tech)-[:SPONSORS {amount: 3000000, percentage: 60}]->(ai);
MATCH (data:Company {name: 'DataInc'}), (pipeline:Project {name: 'Data Pipeline'}) INSERT (data)-[:SPONSORS {amount: 2500000, percentage: 83}]->(pipeline);
MATCH (cloud:Company {name: 'CloudSystems'}), (mobile:Project {name: 'Mobile App'}) INSERT (cloud)-[:SPONSORS {amount: 1500000, percentage: 75}]->(mobile);
MATCH (tech:Company {name: 'TechCorp'}), (security:Project {name: 'Security Audit'}) INSERT (tech)-[:SPONSORS {amount: 500000, percentage: 100}]->(security);
```

**Expected Output:**
- 6 Person nodes created
- 3 Company nodes created
- 4 Project nodes created
- 7 KNOWS relationships created
- 6 WORKS_AT relationships created
- 5 ASSIGNED_TO relationships created
- 4 SPONSORS relationships created
- **Total: 13 nodes, 22 relationships**

### 2.2 Verify Data Insertion

```gql
-- Count nodes by label
MATCH (p:Person) RETURN COUNT(p) AS person_count;
MATCH (c:Company) RETURN COUNT(c) AS company_count;
MATCH (proj:Project) RETURN COUNT(proj) AS project_count;

-- Count relationships by type
MATCH ()-[r:KNOWS]->() RETURN COUNT(r) AS knows_count;
MATCH ()-[r:WORKS_AT]->() RETURN COUNT(r) AS works_at_count;
MATCH ()-[r:ASSIGNED_TO]->() RETURN COUNT(r) AS assigned_to_count;
MATCH ()-[r:SPONSORS]->() RETURN COUNT(r) AS sponsors_count;
```

**Expected Output:**
- person_count: 6
- company_count: 3
- project_count: 4
- knows_count: 7
- works_at_count: 6
- assigned_to_count: 5
- sponsors_count: 4

---

## 3. Property Updates with SET

### 3.1 Update Node Properties Using SET

```gql
-- Update single property
MATCH (p:Person {name: 'Alice Johnson'})
SET p.age = 31;
```

**Expected Output:** 1 property updated

```gql
-- Update multiple properties
MATCH (p:Person {name: 'Bob Smith'})
SET p.age = 26, p.status = 'premium';
```

**Expected Output:** 2 properties updated

```gql
-- Add new property
MATCH (p:Person {name: 'Carol Williams'})
SET p.phone = '+1-555-0123';
```

**Expected Output:** 1 property added

### 3.2 Update Relationship Properties Using MATCH-SET

```gql
-- Update relationship property
MATCH (alice:Person {name: 'Alice Johnson'})-[r:KNOWS]->(bob:Person {name: 'Bob Smith'})
SET r.strength = 'very strong', r.last_contact = '2023-12-01';
```

**Expected Output:** 2 properties updated

```gql
-- Update salary for work relationship
MATCH (carol:Person {name: 'Carol Williams'})-[r:WORKS_AT]->(c:Company)
SET r.salary = 100000, r.promoted = 'yes';
```

**Expected Output:** 2 properties updated

### 3.3 Verify Updates

```gql
-- Verify person updates
MATCH (p:Person {name: 'Alice Johnson'})
RETURN p.name, p.age, p.status;

MATCH (p:Person {name: 'Carol Williams'})
RETURN p.name, p.age, p.phone;

-- Verify relationship updates
MATCH (alice:Person {name: 'Alice Johnson'})-[r:KNOWS]->(bob:Person)
RETURN alice.name, bob.name, r.strength, r.last_contact;
```

**Expected Output:** Updated values should be reflected

---

## 4. CALL and YIELD

### 4.1 List Schemas

```gql
-- Call system procedure to list all schemas
CALL gql.list_schemas();
```

**Expected Output:** List of schemas including /test_schema

### 4.2 List Graphs

```gql
-- List all graphs in current schema
CALL gql.list_graphs();
```

**Expected Output:** List including /test_schema/social_network

### 4.3 Describe Schema

```gql
-- Get schema details
CALL gql.describe_schema('/test_schema');
```

**Expected Output:** Schema metadata

### 4.4 Describe Graph

```gql
-- Get graph details
CALL gql.describe_graph('/test_schema/social_network');
```

**Expected Output:** Graph metadata including node/edge counts

### 4.5 Cache Statistics

```gql
-- Get cache performance stats
CALL gql.cache_stats();
```

**Expected Output:** Cache hit rates, sizes, and statistics

### 4.6 Show Session

```gql
-- Display current session information
CALL gql.show_session();
```

**Expected Output:** Current user, schema, graph, permissions

---

## 5. Simple Pattern Matching

### 5.1 Match All Nodes of a Type

```gql
-- Find all people
MATCH (p:Person)
RETURN p.name, p.age, p.city;
```

**Expected Output:** 6 rows with person details

```gql
-- Find all companies
MATCH (c:Company)
RETURN c.name, c.industry, c.employees;
```

**Expected Output:** 3 rows with company details

### 5.2 Match Specific Node Properties

```gql
-- Find person by name
MATCH (p:Person {name: 'Alice Johnson'})
RETURN p.name, p.age, p.email;
```

**Expected Output:** 1 row with Alice's details

```gql
-- Find people in New York
MATCH (p:Person {city: 'New York'})
RETURN p.name, p.age;
```

**Expected Output:** 2 rows (Alice and Carol)

### 5.3 Match Simple Relationship Patterns

```gql
-- Find who Alice knows
MATCH (alice:Person {name: 'Alice Johnson'})-[:KNOWS]->(friend)
RETURN friend.name;
```

**Expected Output:** 2 rows (Bob and Carol)

```gql
-- Find where people work
MATCH (p:Person)-[:WORKS_AT]->(c:Company)
RETURN p.name, c.name;
```

**Expected Output:** 6 rows showing person-company pairs

```gql
-- Find project assignments
MATCH (p:Person)-[:ASSIGNED_TO]->(proj:Project)
RETURN p.name AS person, proj.name AS project;
```

**Expected Output:** 5 rows showing assignments

### 5.4 Match with Relationship Properties

```gql
-- Find strong friendships
MATCH (p1:Person)-[r:KNOWS {strength: 'strong'}]->(p2:Person)
RETURN p1.name, p2.name, r.since;
```

**Expected Output:** 3 rows with strong relationships

```gql
-- Find high-salary positions
MATCH (p:Person)-[r:WORKS_AT]->(c:Company)
WHERE r.salary > 110000
RETURN p.name, c.name, r.salary, r.role;
```

**Expected Output:** 3 rows (Alice, David, Frank)

---

## 6. Pattern Matching with WHERE and RETURN

### 6.1 Simple WHERE Conditions

```gql
-- Find people older than 28
MATCH (p:Person)
WHERE p.age > 28
RETURN p.name, p.age
ORDER BY p.age;
```

**Expected Output:** 4 rows (Alice: 30, Frank: 32, David: 35, and Carol if age > 28)

```gql
-- Find active people in specific cities
MATCH (p:Person)
WHERE p.city = 'New York' AND p.status = 'active'
RETURN p.name, p.city, p.status;
```

**Expected Output:** 2 rows (Alice and Carol)

```gql
-- Find companies with more than 300 employees
MATCH (c:Company)
WHERE c.employees > 300
RETURN c.name, c.employees, c.revenue;
```

**Expected Output:** 2 rows (TechCorp and CloudSystems)

### 6.2 OR Patterns in WHERE

```gql
-- Find people in San Francisco OR Chicago
MATCH (p:Person)
WHERE p.city = 'San Francisco' OR p.city = 'Chicago'
RETURN p.name, p.city;
```

**Expected Output:** 3 rows (Bob, Eve, David)

```gql
-- Find high-priority or completed projects
MATCH (proj:Project)
WHERE proj.priority = 'high' OR proj.status = 'completed'
RETURN proj.name, proj.priority, proj.status;
```

**Expected Output:** 3 rows (AI Platform, Data Pipeline, and any high priority projects)

### 6.3 AND/OR Combined Patterns

```gql
-- Find active people who are young OR in New York
MATCH (p:Person)
WHERE p.status = 'active' AND (p.age < 30 OR p.city = 'New York')
RETURN p.name, p.age, p.city, p.status;
```

**Expected Output:** 4 rows (Alice, Bob, Carol, Eve)

```gql
-- Find tech or analytics companies with high revenue
MATCH (c:Company)
WHERE (c.industry = 'Technology' OR c.industry = 'Analytics')
  AND c.revenue > 30000000
RETURN c.name, c.industry, c.revenue;
```

**Expected Output:** 1-2 rows (TechCorp and possibly CloudSystems)

### 6.4 Complex Relationship Patterns with WHERE

```gql
-- Find people who know someone in San Francisco
MATCH (p1:Person)-[:KNOWS]->(p2:Person)
WHERE p2.city = 'San Francisco'
RETURN p1.name AS person, p2.name AS friend_in_sf, p2.city;
```

**Expected Output:** 2 rows (Alice→Bob, Bob→Eve)

```gql
-- Find coworkers at TechCorp
MATCH (p1:Person)-[:WORKS_AT]->(c:Company {name: 'TechCorp'}),
      (p2:Person)-[:WORKS_AT]->(c)
WHERE p1.name < p2.name
RETURN p1.name, p2.name, c.name AS company;
```

**Expected Output:** 3 rows (Alice-Bob, Alice-Eve, Bob-Eve)

---

## 7. NEXT Clause

### 7.1 Basic NEXT Usage

```gql
-- Find friends of friends
MATCH (person:Person {name: 'Alice Johnson'})-[:KNOWS]->(friend)
NEXT MATCH (friend)-[:KNOWS]->(fof)
RETURN person.name AS start, friend.name AS intermediate, fof.name AS friend_of_friend;
```

**Expected Output:** Rows showing 2-hop paths from Alice

```gql
-- Find company through person
MATCH (p:Person {name: 'Bob Smith'})
NEXT MATCH (p)-[:WORKS_AT]->(c:Company)
RETURN p.name, c.name AS company, c.industry;
```

**Expected Output:** 1 row (Bob → TechCorp)

### 7.2 Multi-hop Patterns with NEXT

```gql
-- Find people → company → project path
MATCH (p:Person {name: 'Alice Johnson'})-[:WORKS_AT]->(c:Company)
NEXT MATCH (c)-[:SPONSORS]->(proj:Project)
RETURN p.name AS person, c.name AS company, proj.name AS project;
```

**Expected Output:** Rows showing Alice's company's sponsored projects

```gql
-- Three-hop: person → knows → works_at → company
MATCH (p1:Person {name: 'Alice Johnson'})-[:KNOWS]->(p2:Person)
NEXT MATCH (p2)-[:WORKS_AT]->(c:Company)
RETURN p1.name, p2.name, c.name AS company;
```

**Expected Output:** Alice's friends and where they work

### 7.3 NEXT with WHERE Clause

```gql
-- Find friends working at high-revenue companies
MATCH (p1:Person {name: 'Alice Johnson'})-[:KNOWS]->(p2:Person)
NEXT MATCH (p2)-[:WORKS_AT]->(c:Company)
WHERE c.revenue > 50000000
RETURN p1.name, p2.name, c.name, c.revenue;
```

**Expected Output:** Alice's friends at high-revenue companies

```gql
-- Find project paths with allocation filter
MATCH (p:Person)-[:ASSIGNED_TO]->(proj:Project)
WHERE proj.status = 'active'
NEXT MATCH (c:Company)-[:SPONSORS]->(proj)
WHERE c.employees > 300
RETURN p.name, proj.name, c.name;
```

**Expected Output:** Active projects with large company sponsors

---

## 8. String and Date/Time Functions

### 8.1 String Functions

```gql
-- Convert names to uppercase
MATCH (p:Person)
RETURN p.name, upper(p.name) AS name_upper;
```

**Expected Output:** 6 rows with uppercase names

```gql
-- Convert to lowercase and extract substring
MATCH (p:Person)
RETURN p.name,
       lower(p.name) AS name_lower,
       substring(p.name, 0, 5) AS first_5_chars;
```

**Expected Output:** 6 rows with lowercase and substrings

```gql
-- Trim and replace operations
MATCH (c:Company)
RETURN c.name,
       trim(c.name) AS trimmed,
       replace(c.name, 'Inc', 'Incorporated') AS replaced;
```

**Expected Output:** 3 rows with string transformations

```gql
-- String concatenation in WHERE
MATCH (p:Person)
WHERE lower(p.email) LIKE '%example.com%'
RETURN p.name, p.email;
```

**Expected Output:** All 6 people (all have @example.com)

### 8.2 Date/Time Functions

```gql
-- Create duration values
MATCH (p:Person)
RETURN p.name,
       p.joined,
       duration('P1Y') AS one_year;
```

**Expected Output:** 6 rows with duration

```gql
-- Calculate tenure using duration
MATCH (p:Person)-[r:WORKS_AT]->(c:Company)
RETURN p.name,
       c.name,
       r.since,
       duration('P3Y') AS three_years;
```

**Expected Output:** 6 rows with work tenure information

```gql
-- Filter by date ranges
MATCH (p:Person)
WHERE p.joined > '2020-01-01' AND p.joined < '2021-12-31'
RETURN p.name, p.joined
ORDER BY p.joined;
```

**Expected Output:** People who joined between 2020-2021

### 8.3 Combined String and Date Functions

```gql
-- Format output with string and date functions
MATCH (p:Person)-[:WORKS_AT]->(c:Company)
RETURN upper(p.name) AS name,
       lower(c.industry) AS industry,
       substring(p.email, 0, 10) AS email_preview;
```

**Expected Output:** 6 rows with formatted data

```gql
-- Complex filtering with functions
MATCH (p:Person)
WHERE upper(p.city) = 'NEW YORK'
  AND p.joined > '2020-01-01'
RETURN p.name, p.city, p.joined;
```

**Expected Output:** New York residents who joined after 2020

---

## 9. ORDER BY Clause

### 9.1 Simple ORDER BY

```gql
-- Order by age ascending
MATCH (p:Person)
RETURN p.name, p.age
ORDER BY p.age;
```

**Expected Output:** 6 rows sorted by age (youngest first)

```gql
-- Order by age descending
MATCH (p:Person)
RETURN p.name, p.age
ORDER BY p.age DESC;
```

**Expected Output:** 6 rows sorted by age (oldest first)

```gql
-- Order by string property
MATCH (c:Company)
RETURN c.name, c.employees
ORDER BY c.name;
```

**Expected Output:** 3 companies alphabetically sorted

### 9.2 Multiple ORDER BY Columns

```gql
-- Order by city, then by age
MATCH (p:Person)
RETURN p.name, p.city, p.age
ORDER BY p.city, p.age;
```

**Expected Output:** 6 rows sorted by city first, then age within each city

```gql
-- Order by status descending, then name ascending
MATCH (p:Person)
RETURN p.name, p.status, p.age
ORDER BY p.status DESC, p.name;
```

**Expected Output:** Active people first, then inactive, alphabetically within each group

### 9.3 ORDER BY with Expressions

```gql
-- Order by salary from relationship
MATCH (p:Person)-[r:WORKS_AT]->(c:Company)
RETURN p.name, c.name, r.salary
ORDER BY r.salary DESC;
```

**Expected Output:** 6 rows sorted by salary (highest first)

```gql
-- Order by computed values
MATCH (c:Company)
RETURN c.name, c.employees, c.revenue, c.revenue / c.employees AS revenue_per_employee
ORDER BY revenue_per_employee DESC;
```

**Expected Output:** 3 companies sorted by revenue per employee

### 9.4 ORDER BY with LIMIT

```gql
-- Top 3 oldest people
MATCH (p:Person)
RETURN p.name, p.age
ORDER BY p.age DESC
LIMIT 3;
```

**Expected Output:** 3 rows (David, Frank, Alice)

```gql
-- Top 2 highest-paid employees
MATCH (p:Person)-[r:WORKS_AT]->(c:Company)
RETURN p.name, c.name, r.salary
ORDER BY r.salary DESC
LIMIT 2;
```

**Expected Output:** 2 highest salaries

---

## 10. GROUP BY and HAVING

### 10.1 Basic GROUP BY

```gql
-- Count people per city
MATCH (p:Person)
RETURN p.city, COUNT(p) AS person_count
GROUP BY p.city;
```

**Expected Output:** 4-5 rows (one per city with counts)

```gql
-- Average age per city
MATCH (p:Person)
RETURN p.city, AVG(p.age) AS avg_age, COUNT(p) AS population
GROUP BY p.city;
```

**Expected Output:** City-wise statistics

```gql
-- Count employees per company
MATCH (p:Person)-[:WORKS_AT]->(c:Company)
RETURN c.name, COUNT(p) AS employee_count
GROUP BY c.name;
```

**Expected Output:** 3 rows (TechCorp: 3, DataInc: 2, CloudSystems: 1)

### 10.2 GROUP BY with Aggregations

```gql
-- Min, Max, Average age per city
MATCH (p:Person)
RETURN p.city,
       MIN(p.age) AS min_age,
       MAX(p.age) AS max_age,
       AVG(p.age) AS avg_age,
       COUNT(p) AS count
GROUP BY p.city;
```

**Expected Output:** Comprehensive city statistics

```gql
-- Salary statistics per company
MATCH (p:Person)-[r:WORKS_AT]->(c:Company)
RETURN c.name,
       MIN(r.salary) AS min_salary,
       MAX(r.salary) AS max_salary,
       AVG(r.salary) AS avg_salary,
       COUNT(p) AS employee_count
GROUP BY c.name;
```

**Expected Output:** Salary stats for each company

### 10.3 HAVING Clause

```gql
-- Cities with more than 1 person
MATCH (p:Person)
RETURN p.city, COUNT(p) AS person_count
GROUP BY p.city
HAVING COUNT(p) > 1;
```

**Expected Output:** Only cities with 2+ people (New York, San Francisco)

```gql
-- Companies with average salary above 105,000
MATCH (p:Person)-[r:WORKS_AT]->(c:Company)
RETURN c.name, AVG(r.salary) AS avg_salary, COUNT(p) AS employee_count
GROUP BY c.name
HAVING AVG(r.salary) > 105000;
```

**Expected Output:** Companies meeting salary threshold

```gql
-- Cities with high average age (>28)
MATCH (p:Person)
RETURN p.city, AVG(p.age) AS avg_age, COUNT(p) AS population
GROUP BY p.city
HAVING AVG(p.age) > 28;
```

**Expected Output:** Cities with older populations

### 10.4 GROUP BY with ORDER BY

```gql
-- Cities ranked by population
MATCH (p:Person)
RETURN p.city, COUNT(p) AS population
GROUP BY p.city
ORDER BY population DESC;
```

**Expected Output:** Cities sorted by person count

```gql
-- Companies ranked by average salary
MATCH (p:Person)-[r:WORKS_AT]->(c:Company)
RETURN c.name,
       AVG(r.salary) AS avg_salary,
       COUNT(p) AS employees
GROUP BY c.name
ORDER BY avg_salary DESC;
```

**Expected Output:** Companies sorted by average salary

### 10.5 Complex GROUP BY with HAVING and ORDER BY

```gql
-- Status groups with statistics, filtered and sorted
MATCH (p:Person)
RETURN p.status,
       COUNT(p) AS count,
       AVG(p.age) AS avg_age,
       MIN(p.age) AS min_age,
       MAX(p.age) AS max_age
GROUP BY p.status
HAVING COUNT(p) >= 2
ORDER BY avg_age DESC;
```

**Expected Output:** Status groups meeting criteria, sorted by age

```gql
-- Project priority with funding stats
MATCH (c:Company)-[r:SPONSORS]->(proj:Project)
RETURN proj.priority,
       COUNT(proj) AS project_count,
       SUM(r.amount) AS total_funding,
       AVG(r.amount) AS avg_funding
GROUP BY proj.priority
HAVING SUM(r.amount) > 1000000
ORDER BY total_funding DESC;
```

**Expected Output:** Priority groups with significant funding

---

## Additional Test Scenarios

### Multi-hop Relationship Queries

```gql
-- Find colleagues (people working at same company)
MATCH (p1:Person)-[:WORKS_AT]->(c:Company),
      (p2:Person)-[:WORKS_AT]->(c)
WHERE p1.name < p2.name
RETURN p1.name AS person1, p2.name AS person2, c.name AS company;
```

**Expected Output:** Colleague pairs at each company

```gql
-- Find people working on same project
MATCH (p1:Person)-[:ASSIGNED_TO]->(proj:Project),
      (p2:Person)-[:ASSIGNED_TO]->(proj)
WHERE p1.name < p2.name
RETURN p1.name, p2.name, proj.name;
```

**Expected Output:** Team members on shared projects

### Combined Features Query

```gql
-- Complex query combining multiple features
MATCH (p:Person)-[w:WORKS_AT]->(c:Company)
WHERE p.age > 25 AND w.salary > 100000
RETURN upper(p.name) AS name,
       p.age,
       c.name AS company,
       w.salary,
       w.role
ORDER BY w.salary DESC, p.age
LIMIT 5;
```

**Expected Output:** Top 5 high earners with formatted output

```gql
-- Group by with string functions and having
MATCH (p:Person)-[:WORKS_AT]->(c:Company)
RETURN upper(substring(c.industry, 0, 4)) AS industry_code,
       COUNT(p) AS workers,
       AVG(p.age) AS avg_age
GROUP BY c.industry
HAVING COUNT(p) >= 2
ORDER BY workers DESC;
```

**Expected Output:** Industry statistics for categories with 2+ workers

---

## Cleanup

### Remove Test Data (Optional)

```gql
-- Delete all relationships first
MATCH ()-[r]->()
DELETE r;

-- Then delete all nodes
MATCH (n)
DELETE n;

-- Drop graph
DROP GRAPH /test_schema/social_network;

-- Drop schema
DROP SCHEMA /test_schema;
```

---

## Notes

- All queries assume you're in the GraphLite REPL with the session context set
- Query execution times will vary based on system performance
- Some queries may need adjustment based on actual data inserted
- If any query fails, check the error message and verify prerequisites
- Use `CALL gql.show_session();` to verify context at any time

---

## Troubleshooting

**If queries fail with "No graph context":**
```gql
SESSION SET GRAPH /test_schema/social_network;
```

**If data isn't found:**
```gql
-- Verify data exists
MATCH (n) RETURN COUNT(n) AS total_nodes;
MATCH ()-[r]->() RETURN COUNT(r) AS total_relationships;
```

**To reset and start over:**
```gql
-- See cleanup section above
```

---

**End of GQL Guide**
**Last Updated**: November 2025