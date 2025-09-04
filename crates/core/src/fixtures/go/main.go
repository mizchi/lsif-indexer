package main

import "fmt"

type Person struct {
    Name string
    Age  int
}

type Employee struct {
    Person
    Department string
}

func (e *Employee) Introduce() string {
    return fmt.Sprintf("I'm %s, %d years old", e.Name, e.Age)
}

func main() {
    emp := &Employee{
        Person: Person{
            Name: "Bob",
            Age:  25,
        },
        Department: "Sales",
    }
    fmt.Println(emp.Introduce())
}

func Add(a, b int) int {
    return a + b
}