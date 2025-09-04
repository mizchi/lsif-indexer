interface Person {
    name: string;
    age: number;
}

class Employee implements Person {
    constructor(
        public name: string,
        public age: number,
        public department: string
    ) {}

    introduce(): string {
        return `I'm ${this.name}, ${this.age} years old`;
    }
}

function createEmployee(name: string, age: number): Employee {
    return new Employee(name, age, "Engineering");
}

export { Person, Employee, createEmployee };