package com.example.demo.dto;

public class UserResponse {

    private Long id;
    private String name;
    private String email;
    private Integer age;
    private Boolean active;

    public UserResponse(Long id, String name, String email, Integer age, Boolean active) {
        this.id = id;
        this.name = name;
        this.email = email;
        this.age = age;
        this.active = active;
    }

    public Long getId() {
        return id;
    }

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
}
