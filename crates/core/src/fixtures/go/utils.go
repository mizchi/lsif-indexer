package main

import "strings"

func FormatName(name string) string {
    return "Hello, " + name + "!"
}

func ToUpper(s string) string {
    return strings.ToUpper(s)
}

type Calculator struct{}

func (c *Calculator) Add(a, b int) int {
    return a + b
}