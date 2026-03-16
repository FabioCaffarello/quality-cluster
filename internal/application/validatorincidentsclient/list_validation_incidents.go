package validatorincidentsclient

import (
	"context"

	"internal/application/ports"
	validatorincidentscontracts "internal/application/validatorincidents/contracts"
	"internal/shared/problem"
)

type ListValidationIncidentsUseCase struct {
	gateway ports.ValidatorIncidentsGateway
}

func NewListValidationIncidentsUseCase(gateway ports.ValidatorIncidentsGateway) *ListValidationIncidentsUseCase {
	return &ListValidationIncidentsUseCase{gateway: gateway}
}

func (uc *ListValidationIncidentsUseCase) Execute(ctx context.Context, query validatorincidentscontracts.ListValidationIncidentsQuery) (validatorincidentscontracts.ListValidationIncidentsReply, *problem.Problem) {
	if uc == nil || uc.gateway == nil {
		return validatorincidentscontracts.ListValidationIncidentsReply{}, problem.New(problem.Unavailable, "validation incidents service is unavailable")
	}

	query = query.Normalize()
	if prob := query.Validate(); prob != nil {
		return validatorincidentscontracts.ListValidationIncidentsReply{}, prob
	}

	return uc.gateway.ListValidationIncidents(ctx, query)
}
