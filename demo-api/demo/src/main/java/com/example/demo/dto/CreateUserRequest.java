package com.example.demo.dto;

import jakarta.validation.constraints.*;
import com.example.demo.validation.CompanyEmail;
import com.example.demo.validation.ValidationMessages;

public class CreateUserRequest {

    @NotBlank(message = ValidationMessages.NAME_REQUIRED)
    @Size(min = 3, max = 120)
    private String name;

    @NotBlank(message = ValidationMessages.EMAIL_REQUIRED)
    @Email(message = ValidationMessages.INVALID_EMAIL)
    @CompanyEmail
    private String email;

    @NotNull
    @Min(value = 18, message = ValidationMessages.AGE_MINIMUM)
    @Max(value = 120)
    private Integer age;

    public String getName() {
        return name;
    }

    public String getEmail() {
        return email;
    }

    public Integer getAge() {
        return age;
    }

    public void setName(String name) {
        this.name = name;
    }

    public void setEmail(String email) {
        this.email = email;
    }

    public void setAge(Integer age) {
        this.age = age;
    }
}
