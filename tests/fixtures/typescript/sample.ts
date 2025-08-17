// Sample TypeScript file for testing LSP integration

interface User {
    id: number;
    name: string;
    email: string;
    isActive: boolean;
}

class UserService {
    private users: Map<number, User>;
    
    constructor() {
        this.users = new Map();
    }
    
    addUser(user: User): void {
        this.users.set(user.id, user);
    }
    
    getUser(id: number): User | undefined {
        return this.users.get(id);
    }
    
    getAllUsers(): User[] {
        return Array.from(this.users.values());
    }
    
    updateUser(id: number, updates: Partial<User>): boolean {
        const user = this.users.get(id);
        if (!user) {
            return false;
        }
        
        const updatedUser = { ...user, ...updates };
        this.users.set(id, updatedUser);
        return true;
    }
    
    deleteUser(id: number): boolean {
        return this.users.delete(id);
    }
}

// Utility functions
export function validateEmail(email: string): boolean {
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    return emailRegex.test(email);
}

export function formatUserName(firstName: string, lastName: string): string {
    return `${firstName} ${lastName}`.trim();
}

// Generic type example
type AsyncResult<T> = Promise<{ data: T; error?: string }>;

async function fetchUserData(id: number): AsyncResult<User> {
    // Simulated async operation
    return new Promise((resolve) => {
        setTimeout(() => {
            resolve({
                data: {
                    id,
                    name: "Test User",
                    email: "test@example.com",
                    isActive: true
                }
            });
        }, 100);
    });
}

// Enum example
enum UserRole {
    Admin = "ADMIN",
    User = "USER",
    Guest = "GUEST"
}

// Type guard
function isAdmin(role: UserRole): boolean {
    return role === UserRole.Admin;
}

// Higher-order function
function withLogging<T extends (...args: any[]) => any>(fn: T): T {
    return ((...args: Parameters<T>) => {
        console.log(`Calling ${fn.name} with args:`, args);
        const result = fn(...args);
        console.log(`Result:`, result);
        return result;
    }) as T;
}

// Main function to test everything
async function main(): Promise<void> {
    const service = new UserService();
    
    // Add users
    service.addUser({
        id: 1,
        name: "Alice",
        email: "alice@example.com",
        isActive: true
    });
    
    service.addUser({
        id: 2,
        name: "Bob",
        email: "bob@example.com",
        isActive: false
    });
    
    // Test operations
    const user = service.getUser(1);
    console.log("User 1:", user);
    
    const allUsers = service.getAllUsers();
    console.log("All users:", allUsers);
    
    // Test async operation
    const asyncResult = await fetchUserData(3);
    console.log("Async user:", asyncResult.data);
}

// Export for testing
export { UserService, UserRole, isAdmin, withLogging, main };