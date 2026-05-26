package com.example.demo.exception;

/**
 * Application-level error codes paired with default messages.
 * Use these from services/controllers to ensure message consistency
 * and make it easier to internationalize or map to HTTP statuses later.
 */
public enum ErrorCode {

    USER_NOT_FOUND("User not found"),
    EMAIL_ALREADY_EXISTS("Email already exists"),
    UNDERAGE_NOT_ALLOWED("Underage users are not allowed"),
    AGE_TOO_YOUNG("User must be at least 18 years old"),
    INACTIVE_CANNOT_UPDATE_EMAIL("Inactive users cannot update email"),
    ACTIVE_CANNOT_BE_DELETED("Active users cannot be deleted");

    private final String defaultMessage;

    ErrorCode(String defaultMessage) {
        this.defaultMessage = defaultMessage;
    }

    public String getDefaultMessage() {
        return defaultMessage;
    }
}
