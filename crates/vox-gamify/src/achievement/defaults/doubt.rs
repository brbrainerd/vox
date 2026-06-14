use crate::achievement::{Achievement, AchievementCategory, AchievementId};

pub fn doubt_achievements() -> Vec<Achievement> {
    vec![
        Achievement {
            id: AchievementId("healthy_skepticism".to_string()),
            name: "Healthy Skepticism".to_string(),
            description: "Correctly flag a task as suspect (Resolution Agent Overruled AI).".to_string(),
            icon: "👁️".to_string(),
            category: AchievementCategory::Efficiency,
            xp_reward: 50,
            crystal_reward: 5,
            icon_slug: "eye_open".to_string(),
            secret: false,
        },
        Achievement {
            id: AchievementId("eagle_eye".to_string()),
            name: "Eagle Eye".to_string(),
            description: "Correctly flag 10 suspect tasks.".to_string(),
            icon: "🦅".to_string(),
            category: AchievementCategory::Efficiency,
            xp_reward: 250,
            crystal_reward: 25,
            icon_slug: "eagle".to_string(),
            secret: false,
        },
        Achievement {
            id: AchievementId("blind_faith".to_string()),
            name: "Blind Faith?".to_string(),
            description: "Unjustifiably flag a correct task as suspect 5 times.".to_string(),
            icon: "🙈".to_string(),
            category: AchievementCategory::Efficiency,
            xp_reward: 10,
            crystal_reward: 0,
            icon_slug: "blindfold".to_string(),
            secret: true,
        },
        Achievement {
            id: AchievementId("incorruptible".to_string()),
            name: "Incorruptible".to_string(),
            description: "Flag a task as suspect and be validated by the Resolution Agent when the model was being obsequious.".to_string(),
            icon: "🛡️".to_string(),
            category: AchievementCategory::Security,
            xp_reward: 150,
            crystal_reward: 15,
            icon_slug: "shield_check".to_string(),
            secret: false,
        },
        Achievement {
            id: AchievementId("internal_affairs".to_string()),
            name: "Internal Affairs".to_string(),
            description: "Expose an agent's obsequious behavior via the doubt loop.".to_string(),
            icon: "🕵️".to_string(),
            category: AchievementCategory::Social,
            xp_reward: 200,
            crystal_reward: 20,
            icon_slug: "detective".to_string(),
            secret: false,
        },
    ]
}

#[cfg(test)]
mod semcov_wave1d_tests {
    #![allow(unused_imports)]
    use super::*;

    #[test]
    fn doubt_achievements_table_has_expected_shape() {
        let a = doubt_achievements();
        assert_eq!(a.len(), 5);
        let ids: Vec<&str> = a.iter().map(|x| x.id.0.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "healthy_skepticism",
                "eagle_eye",
                "blind_faith",
                "incorruptible",
                "internal_affairs"
            ]
        );
        let secret: Vec<&Achievement> = a.iter().filter(|x| x.secret).collect();
        assert_eq!(secret.len(), 1);
        assert_eq!(secret[0].id.0, "blind_faith");
        assert_eq!(secret[0].xp_reward, 10);
        assert_eq!(secret[0].crystal_reward, 0);
    }
}
