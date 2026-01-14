#!/bin/bash

# Valid alias
alias ll='ls -la'

# Intentional syntax error - unclosed quote
alias broken="unclosed quote

# Valid function
function test_func() {
    echo "hello"
}
