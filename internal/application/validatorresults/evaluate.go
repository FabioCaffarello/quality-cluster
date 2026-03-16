package validatorresults

import (
	"encoding/json"
	"fmt"
	"strings"
	"time"

	dataplaneapp "internal/application/dataplane"
	sharedruntime "internal/application/runtimecontracts"
	validatorcontracts "internal/application/validatorresults/contracts"
	configdomain "internal/domain/configctl"
	"internal/shared/problem"
)

func Evaluate(runtime configdomain.RuntimeProjection, message dataplaneapp.Message, processedAt time.Time) (validatorcontracts.ValidationResultRecord, *problem.Problem) {
	if prob := message.Validate(); prob != nil {
		return validatorcontracts.ValidationResultRecord{}, prob
	}

	runtimeScope := runtime.Scope.Normalize()
	if runtimeScope.Kind != strings.TrimSpace(message.Binding.Scope.Kind) || runtimeScope.Key != strings.TrimSpace(message.Binding.Scope.Key) {
		return validatorcontracts.ValidationResultRecord{}, problem.New(problem.Conflict, "validator runtime scope does not match data plane message")
	}
	if strings.TrimSpace(runtime.VersionID) != strings.TrimSpace(message.Binding.Config.VersionID) {
		return validatorcontracts.ValidationResultRecord{}, problem.New(problem.Conflict, "validator runtime version does not match data plane message")
	}

	payload, prob := decodePayloadObject(message.Payload)
	if prob != nil {
		return validatorcontracts.ValidationResultRecord{}, prob
	}
	if processedAt.IsZero() {
		processedAt = time.Now().UTC()
	}

	result := validatorcontracts.ValidationResultRecord{
		ProcessingKey: validatorcontracts.BuildValidationProcessingKey(
			message.Metadata.MessageID,
			validatorcontracts.ValidationBindingRecord{
				Name:  message.Binding.Name,
				Topic: message.Binding.Topic,
				Scope: sharedruntime.ScopeRecord{
					Kind: message.Binding.Scope.Kind,
					Key:  message.Binding.Scope.Key,
				},
			},
			validatorcontracts.ValidationConfigRecord{
				SetID:              runtime.ConfigSetID,
				Key:                runtime.ConfigKey,
				VersionID:          runtime.VersionID,
				Version:            runtime.Version,
				DefinitionChecksum: runtime.DefinitionChecksum,
			},
		),
		MessageID:     message.Metadata.MessageID,
		CorrelationID: message.Metadata.CorrelationID,
		Binding: validatorcontracts.ValidationBindingRecord{
			Name:  message.Binding.Name,
			Topic: message.Binding.Topic,
			Scope: sharedruntime.ScopeRecord{
				Kind: message.Binding.Scope.Kind,
				Key:  message.Binding.Scope.Key,
			},
		},
		Config: validatorcontracts.ValidationConfigRecord{
			SetID:              runtime.ConfigSetID,
			Key:                runtime.ConfigKey,
			VersionID:          runtime.VersionID,
			Version:            runtime.Version,
			DefinitionChecksum: runtime.DefinitionChecksum,
		},
		Status:      validatorcontracts.ValidationStatusPassed,
		ProcessedAt: processedAt.UTC(),
	}

	for _, rule := range runtime.Rules {
		violation, violated := evaluateRule(rule, payload)
		if !violated {
			continue
		}
		result.Status = validatorcontracts.ValidationStatusFailed
		result.Violations = append(result.Violations, violation)
	}

	if prob := result.Validate(); prob != nil {
		return validatorcontracts.ValidationResultRecord{}, prob
	}

	return result, nil
}

func decodePayloadObject(payload []byte) (map[string]any, *problem.Problem) {
	var object map[string]any
	if err := json.Unmarshal(payload, &object); err != nil {
		return nil, problem.Wrap(err, problem.InvalidArgument, "data plane payload must be a JSON object")
	}
	if object == nil {
		return nil, problem.New(problem.InvalidArgument, "data plane payload must be a JSON object")
	}
	return object, nil
}

func evaluateRule(rule configdomain.Rule, payload map[string]any) (validatorcontracts.ViolationRecord, bool) {
	field := strings.TrimSpace(rule.Field)
	value, exists := payload[field]
	severity := strings.TrimSpace(string(rule.Severity))
	if severity == "" {
		severity = string(configdomain.RuleSeverityError)
	}

	switch rule.Operator {
	case configdomain.RuleOperatorRequired:
		if exists && value != nil {
			return validatorcontracts.ViolationRecord{}, false
		}
		return newViolation(rule, severity, "field is required"), true
	case configdomain.RuleOperatorNotEmpty:
		if exists && !isEmptyValue(value) {
			return validatorcontracts.ViolationRecord{}, false
		}
		return newViolation(rule, severity, "field must not be empty"), true
	case configdomain.RuleOperatorEquals:
		if exists && strings.TrimSpace(fmt.Sprint(value)) == strings.TrimSpace(rule.ExpectedValue) {
			return validatorcontracts.ViolationRecord{}, false
		}
		return newViolation(rule, severity, "field must equal expected value"), true
	default:
		return newViolation(rule, severity, "rule operator is unsupported"), true
	}
}

func newViolation(rule configdomain.Rule, severity, message string) validatorcontracts.ViolationRecord {
	return validatorcontracts.ViolationRecord{
		Rule:     strings.TrimSpace(rule.Name),
		Field:    strings.TrimSpace(rule.Field),
		Operator: strings.TrimSpace(string(rule.Operator)),
		Severity: severity,
		Message:  message,
	}
}

func isEmptyValue(value any) bool {
	switch typed := value.(type) {
	case nil:
		return true
	case string:
		return strings.TrimSpace(typed) == ""
	case []any:
		return len(typed) == 0
	case map[string]any:
		return len(typed) == 0
	default:
		return false
	}
}
