import React from 'react';
import clsx from 'clsx';
import Link from '@docusaurus/Link';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import Layout from '@theme/Layout';
import styles from './index.module.css';
import Heading from '@theme/Heading';

function HomepageHeader() {
  const {siteConfig} = useDocusaurusContext();
  return (
    <header className={clsx('hero hero--primary', styles.heroBanner)}>
      <div className="container">
        <Heading as="h1" className="hero__title">
          {siteConfig.title}
        </Heading>
        <p className="hero__subtitle">{siteConfig.tagline}</p>
        <div className={styles.buttons}>
          <Link
            className="button button--secondary button--lg"
            to="/docs/">
            Get Started →
          </Link>
          <Link
            className="button button--secondary button--lg"
            to="https://github.com/your-org/aeterna">
            GitHub
          </Link>
        </div>
      </div>
    </header>
  );
}

export default function Home(): JSX.Element {
  const {siteConfig} = useDocusaurusContext();
  return (
    <Layout
      title={siteConfig.title}
      description="Universal Memory & Knowledge Framework for AI Agents">
      <HomepageHeader />
      <main>
        <section className={styles.features}>
          <div className="container">
            <div className="row">
              {[
                {
                  title: '7-Layer Memory',
                  description: (
                    <>
                      Hierarchical memory system spanning from working memory through
                      institutional knowledge, giving agents persistent context.
                    </>
                  ),
                },
                {
                  title: 'Git Knowledge Repository',
                  description: (
                    <>
                      Version-controlled knowledge base for ADRs, patterns, and
                      constraints, enabling structured organizational learning.
                    </>
                  ),
                },
                {
                  title: 'Enterprise Governance',
                  description: (
                    <>
                      Multi-tenant RBAC with Cedar policies to ensure compliance
                      and secure access across teams and projects.
                    </>
                  ),
                },
                {
                  title: 'Plug-in Adapters',
                  description: (
                    <>
                      Flexible architecture allowing you to swap storage backends,
                      embedding models, and LLM providers without lock-in.
                    </>
                  ),
                },
                {
                  title: 'MCP Tool Interface',
                  description: (
                    <>
                      Standardized Model Context Protocol interface for seamless
                      integration with modern AI agent frameworks.
                    </>
                  ),
                },
                {
                  title: 'Kubernetes Native',
                  description: (
                    <>
                      Built for scale with a production-ready Helm chart, ensuring
                      reliable deployment in cloud-native environments.
                    </>
                  ),
                },
              ].map((props, idx) => (
                <div key={idx} className={clsx('col col--4')}>
                  <div className="text--center padding-horiz--md">
                    <Heading as="h3">{props.title}</Heading>
                    <p>{props.description}</p>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </section>
      </main>
    </Layout>
  );
}
