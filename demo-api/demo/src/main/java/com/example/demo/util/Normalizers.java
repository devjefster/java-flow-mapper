package com.example.demo.util;

/**
 * Small helpers for normalizing common values used across services.
 */
public final class Normalizers {

    private Normalizers() {}

    public static String normalizeEmail(String email) {
        if (email == null) return null;
        return email.trim().toLowerCase();
    }
}
