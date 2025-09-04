class Person:
    def __init__(self, name: str, age: int):
        self.name = name
        self.age = age

class Employee(Person):
    def __init__(self, name: str, age: int, department: str):
        super().__init__(name, age)
        self.department = department
    
    def introduce(self) -> str:
        return f"I'm {self.name}, {self.age} years old"

def create_employee(name: str, age: int) -> Employee:
    return Employee(name, age, "Marketing")

def main():
    emp = create_employee("Charlie", 35)
    print(emp.introduce())

if __name__ == "__main__":
    main()