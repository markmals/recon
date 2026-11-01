#import "ReconAccessorsInternal.h"

static ReconConstraintFacts *constraintFacts(NSLayoutConstraint *constraint) {
    ReconConstraintFacts *facts = [ReconConstraintFacts new];
    facts.firstItem = constraint.firstItem;
    facts.secondItem = constraint.secondItem;
    facts.firstAttribute = constraint.firstAttribute;
    facts.secondAttribute = constraint.secondAttribute;
    facts.relation = constraint.relation;
    facts.multiplier = constraint.multiplier;
    facts.constant = constraint.constant;
    facts.priority = constraint.priority;
    facts.active = constraint.isActive;
    facts.identifier = constraint.identifier;
    return facts;
}

ReconReadStatus ReconReadConstraint(id constraint, ReconConstraintFacts **value) {
    *value = nil;
    @try {
        if (![constraint isKindOfClass:NSLayoutConstraint.class]) return ReconReadStatusUnavailable;
        *value = constraintFacts(constraint);
        return ReconReadStatusValue;
    } @catch (id exception) {
        return ReconReadStatusFailed;
    }
}

static ReconTransformFacts *transformFacts(CATransform3D matrix) {
    ReconTransformFacts *facts = [ReconTransformFacts new];
    facts.matrix = @[ @(matrix.m11), @(matrix.m12), @(matrix.m13), @(matrix.m14),
                      @(matrix.m21), @(matrix.m22), @(matrix.m23), @(matrix.m24),
                      @(matrix.m31), @(matrix.m32), @(matrix.m33), @(matrix.m34),
                      @(matrix.m41), @(matrix.m42), @(matrix.m43), @(matrix.m44) ];
    return facts;
}

ReconReadStatus ReconReadLayerTransform(id layer, ReconTransformFacts **value) {
    *value = nil;
    @try {
        if (!ReconIsKindOf(layer, ReconNativeClassLayer)) return ReconReadStatusUnavailable;
        *value = transformFacts([(CALayer *)layer transform]);
        return ReconReadStatusValue;
    } @catch (id exception) {
        return ReconReadStatusFailed;
    }
}
