package ports

import (
	"context"

	validatorincidentscontracts "internal/application/validatorincidents/contracts"
	"internal/shared/problem"
)

type ValidatorIncidentsGateway interface {
	ListValidationIncidents(context.Context, validatorincidentscontracts.ListValidationIncidentsQuery) (validatorincidentscontracts.ListValidationIncidentsReply, *problem.Problem)
}
