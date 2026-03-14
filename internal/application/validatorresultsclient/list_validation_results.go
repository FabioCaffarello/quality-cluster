package validatorresultsclient

import (
	"context"

	"internal/application/ports"
	validatorresultscontracts "internal/application/validatorresults/contracts"
	"internal/shared/problem"
)

type ListValidationResultsUseCase struct {
	gateway ports.ValidatorResultsGateway
}

func NewListValidationResultsUseCase(gateway ports.ValidatorResultsGateway) *ListValidationResultsUseCase {
	return &ListValidationResultsUseCase{gateway: gateway}
}

func (uc *ListValidationResultsUseCase) Execute(ctx context.Context, query validatorresultscontracts.ListValidationResultsQuery) (validatorresultscontracts.ListValidationResultsReply, *problem.Problem) {
	if uc == nil || uc.gateway == nil {
		return validatorresultscontracts.ListValidationResultsReply{}, problem.New(problem.Unavailable, "validation results service is unavailable")
	}

	query = query.Normalize()
	if prob := query.Validate(); prob != nil {
		return validatorresultscontracts.ListValidationResultsReply{}, prob
	}

	return uc.gateway.ListValidationResults(ctx, query)
}
