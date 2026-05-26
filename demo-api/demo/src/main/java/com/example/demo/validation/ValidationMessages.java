package com.example.demo.validation;

/**
 * Central place for validation message constants so messages are consistent
 * and easier to refactor or localize.
 */
public final class ValidationMessages {

    private ValidationMessages() {}

    public static final String NAME_REQUIRED = "Name is required";
    public static final String EMAIL_REQUIRED = "Email is required";
    public static final String INVALID_EMAIL = "Invalid email";
    public static final String AGE_MINIMUM = "User must be at least 18 years old";
}
