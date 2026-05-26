package com.example.demo.service;

import com.example.demo.dto.CreateUserRequest;
import com.example.demo.dto.UpdateUserRequest;
import com.example.demo.dto.UserResponse;
import com.example.demo.entity.User;
import com.example.demo.exception.BusinessException;
import com.example.demo.exception.ErrorCode;
import com.example.demo.util.Normalizers;
import com.example.demo.repository.UserRepository;
import org.springframework.stereotype.Service;

import java.util.ArrayList;
import java.util.List;

@Service
public class UserService {

    private final UserRepository repository;

    public UserService(UserRepository repository) {
        this.repository = repository;
    }

    public UserResponse create(CreateUserRequest request) {

        validateEmailUniqueness(request.getEmail());

        // Flow decision
        // Reject underage accounts. This is a domain rule enforced at creation time.
        if (request.getAge() < 18) {
            throw new BusinessException(ErrorCode.UNDERAGE_NOT_ALLOWED);
        }

        User user = new User(
                request.getName().trim(),
                Normalizers.normalizeEmail(request.getEmail()),
                request.getAge(),
                true
        );

        // Persist the normalized entity
        User savedUser = repository.save(user);

        return toResponse(savedUser);
    }

    public List<UserResponse> findAll() {
        return repository.findAll()
                .stream()
                .map(this::toResponse)
                .toList();
    }

    public List<UserResponse> findActive() {
        List<User> users = repository.findAll();
        List<UserResponse> responses = new ArrayList<>();

        for (User user : users) {
            if (Boolean.TRUE.equals(user.getActive())) {
                responses.add(toResponse(user));
            }
        }

        responses.forEach(this::requireResponseEmail);

        return responses;
    }

    public UserResponse findById(Long id) {

        User user = findUserOrThrow(id);

        return toResponse(user);
    }

    public UserResponse update(Long id, UpdateUserRequest request) {

        User user = findUserOrThrow(id);

        // Business rule: inactive users are not allowed to change email. We check
        // the stored active flag rather than trusting the incoming request.
        if (Boolean.FALSE.equals(user.getActive()) && request.getEmail() != null) {
            throw new BusinessException(ErrorCode.INACTIVE_CANNOT_UPDATE_EMAIL);
        }

        if (request.getEmail() != null &&
                !request.getEmail().equalsIgnoreCase(user.getEmail())) {

            validateEmailUniqueness(request.getEmail());

            user.setEmail(Normalizers.normalizeEmail(request.getEmail()));
        }

        if (request.getName() != null) {
            user.setName(request.getName().trim());
        }

        if (request.getAge() != null) {
            // Enforce minimum age on updates as well.
            if (request.getAge() < 18) {
                throw new BusinessException(ErrorCode.AGE_TOO_YOUNG);
            }

            user.setAge(request.getAge());
        }

        if (request.getActive() != null) {
            user.setActive(request.getActive());
        }

        User updatedUser = repository.save(user);

        return toResponse(updatedUser);
    }

    public void delete(Long id) {

        User user = findUserOrThrow(id);

        // Constraint: only inactive users can be removed to prevent accidental
        // deletion of accounts in use.
        if (Boolean.TRUE.equals(user.getActive())) {
            throw new BusinessException(ErrorCode.ACTIVE_CANNOT_BE_DELETED);
        }

        repository.delete(user);
    }

    private User findUserOrThrow(Long id) {
        return repository.findById(id)
                .orElseThrow(() -> new BusinessException(ErrorCode.USER_NOT_FOUND));
    }

    private void validateEmailUniqueness(String email) {

        String normalized = Normalizers.normalizeEmail(email);
        if (repository.existsByEmail(normalized)) {
            throw new BusinessException(ErrorCode.EMAIL_ALREADY_EXISTS);
        }
    }

    private void requireResponseEmail(UserResponse response) {
        if (response.getEmail() == null) {
            throw new IllegalStateException("Active user response missing email");
        }
    }

    private UserResponse toResponse(User user) {
        return new UserResponse(
                user.getId(),
                user.getName(),
                user.getEmail(),
                user.getAge(),
                user.getActive()
        );
    }
}
