package com.example.demo.dto;

import jakarta.validation.constraints.*;
import com.example.demo.validation.ValidationMessages;

public class UpdateUserRequest {

    @Size(min = 3, max = 120)
    private String name;

    @Email(message = ValidationMessages.INVALID_EMAIL)
    private String email;

    @Min(value = 18, message = ValidationMessages.AGE_MINIMUM)
    @Max(value = 120)
    private Integer age;

    private Boolean active;

    public String getName() {
        return name;
    }

    public String getEmail() {
        return email;
    }

    public Integer getAge() {
        return age;
    }

    public Boolean getActive() {
        return active;
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

    public void setActive(Boolean active) {
        this.active = active;
    }
}
